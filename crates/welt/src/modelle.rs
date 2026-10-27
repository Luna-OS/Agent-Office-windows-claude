//! Die in Blender gebauten Modelle (`crates/welt/modelle/*.glb`): Küche, Lounge-Möbel, Pflanzen und
//! die Kleinigkeiten auf den Tischen. Wie im Original sind sie farblos modelliert und werden nach
//! ihren Materialnamen angemalt ("Pot", "Leaf", "Cabinet" …); einzelne Stücke wählt man nach dem
//! Namen ihres obersten Knotens ("monstera", "sofa", "mug" …).
//!
//! kiss3ds eigener glTF-Lader verliert Knoten- und Materialnamen, darum lesen wir die Dateien hier
//! selbst mit dem `gltf`-Crate und legen pro Material ein Netz an (einmal je Datei, dann geteilt).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kiss3d::glamx::{Mat4, Quat as GQuat};
use kiss3d::prelude::*;
use kiss3d::resource::GpuMesh3d;
use kiss3d::resource::vertex_index::VertexIndex;

/// Die Modell-Dateien.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Datei {
    Kueche,
    Lounge,
    Pflanzen,
    Tischkram,
}

impl Datei {
    fn bytes(self) -> &'static [u8] {
        match self {
            Datei::Kueche => include_bytes!("../modelle/kitchen.glb"),
            Datei::Lounge => include_bytes!("../modelle/lounge.glb"),
            Datei::Pflanzen => include_bytes!("../modelle/plants.glb"),
            Datei::Tischkram => include_bytes!("../modelle/desk_props.glb"),
        }
    }
}

/// Ein Netz eines Stücks: in Koordinaten seines obersten Knotens, mit dem Namen seines Materials.
struct Netz {
    mesh: Rc<RefCell<GpuMesh3d>>,
    material: String,
}

/// Ein Stück: der oberste Knoten mit seiner Lage in der Datei, und alle Netze darunter.
struct Stueck {
    lage: Mat4,
    netze: Vec<Netz>,
}

type Geladen = HashMap<String, Stueck>;
/// Punkte, Normalen und Dreiecke eines Materials, bevor sie ein Netz werden.
type Roh = (Vec<Vec3>, Vec<Vec3>, Vec<[VertexIndex; 3]>);

thread_local! {
    static CACHE: RefCell<HashMap<Datei, Rc<Geladen>>> = RefCell::new(HashMap::new());
}

fn laden(d: Datei) -> Rc<Geladen> {
    if let Some(g) = CACHE.with_borrow(|c| c.get(&d).cloned()) {
        return g;
    }
    let g = Rc::new(lesen(d.bytes()).unwrap_or_else(|e| {
        log::warn!("Modell {d:?} ließ sich nicht laden: {e}");
        HashMap::new()
    }));
    CACHE.with_borrow_mut(|c| c.insert(d, g.clone()));
    g
}

fn matrix(n: &::gltf::Node) -> Mat4 {
    let (t, r, s) = n.transform().decomposed();
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(s),
        GQuat::from_array(r),
        Vec3::from_array(t),
    )
}

fn lesen(bytes: &[u8]) -> Result<Geladen, ::gltf::Error> {
    let (doc, buffers, _) = ::gltf::import_slice(bytes)?;
    let mut alle = HashMap::new();
    let Some(szene) = doc.default_scene().or_else(|| doc.scenes().next()) else {
        return Ok(alle);
    };
    for wurzel in szene.nodes() {
        let name = wurzel.name().unwrap_or_default().to_string();
        // Pro Material sammeln, damit jedes Stück wenige Netze hat.
        let mut je_material: HashMap<String, Roh> = HashMap::new();
        let mut stapel = vec![(wurzel.clone(), Mat4::IDENTITY)];
        while let Some((n, eltern)) = stapel.pop() {
            // Unterhalb der Wurzel zählen die Lagen der Kinder; die der Wurzel selbst steht in `lage`.
            let m = if n.index() == wurzel.index() {
                Mat4::IDENTITY
            } else {
                eltern * matrix(&n)
            };
            if let Some(mesh) = n.mesh() {
                for prim in mesh.primitives() {
                    let r = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
                    let (Some(pos), Some(idx)) = (r.read_positions(), r.read_indices()) else {
                        continue;
                    };
                    let normalen: Vec<[f32; 3]> =
                        r.read_normals().map(|n| n.collect()).unwrap_or_default();
                    let mat = prim.material().name().unwrap_or("").to_string();
                    let e = je_material.entry(mat).or_default();
                    let basis = e.0.len() as VertexIndex;
                    let normal_m = m.inverse().transpose();
                    for (i, p) in pos.enumerate() {
                        e.0.push(m.transform_point3(Vec3::from_array(p)));
                        let nn = normalen
                            .get(i)
                            .map(|n| {
                                normal_m
                                    .transform_vector3(Vec3::from_array(*n))
                                    .normalize_or_zero()
                            })
                            .unwrap_or(Vec3::Y);
                        e.1.push(nn);
                    }
                    let idx: Vec<u32> = idx.into_u32().collect();
                    for t in idx.as_chunks::<3>().0 {
                        e.2.push([
                            basis + t[0] as VertexIndex,
                            basis + t[1] as VertexIndex,
                            basis + t[2] as VertexIndex,
                        ]);
                    }
                }
            }
            for k in n.children() {
                stapel.push((k, m));
            }
        }
        let netze = je_material
            .into_iter()
            .map(|(material, (punkte, normalen, flaechen))| Netz {
                mesh: Rc::new(RefCell::new(GpuMesh3d::new(
                    punkte,
                    flaechen,
                    Some(normalen),
                    None,
                    false,
                ))),
                material,
            })
            .collect();
        alle.insert(
            name,
            Stueck {
                lage: matrix(&wurzel),
                netze,
            },
        );
    }
    Ok(alle)
}

/// Baut ein Stück einer Datei als Kind von `eltern` ein, angemalt nach Materialnamen. Ohne `an_ort`
/// liegt der Ursprung des Stücks im Ursprung des Knotens (sonst an seiner Stelle in der Datei).
/// Fehlt das Stück, kommt eine leere Gruppe zurück: die Welt läuft auch ohne.
pub fn stueck(
    eltern: &mut SceneNode3d,
    d: Datei,
    name: &str,
    an_ort: bool,
    farbe: impl Fn(&str) -> Color,
) -> SceneNode3d {
    let mut g = eltern.add_group();
    let geladen = laden(d);
    let Some(s) = geladen.get(name) else {
        log::warn!("Modell {d:?} hat kein Stück '{name}'");
        return g;
    };
    if an_ort {
        let (skala, dreh, ort) = s.lage.to_scale_rotation_translation();
        g.set_local_scale(skala.x, skala.y, skala.z);
        g.set_rotation(dreh);
        g.set_position(ort);
    }
    for n in &s.netze {
        let mut k = g.add_mesh(n.mesh.clone(), Vec3::ONE);
        k.set_color(farbe(&n.material));
        k.set_roughness(0.8);
    }
    g
}

/// Alle Stücke einer Datei an ihren Stellen (z. B. die ganze Küche).
pub fn alles(eltern: &mut SceneNode3d, d: Datei, farbe: impl Fn(&str) -> Color) -> SceneNode3d {
    let mut g = eltern.add_group();
    let namen: Vec<String> = laden(d).keys().cloned().collect();
    for n in namen {
        stueck(&mut g, d, &n, true, &farbe);
    }
    g
}
