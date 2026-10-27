//! Minimalbeispiel: prüft, dass kiss3d im Browser (wasm32/WebGL) rendert.
use kiss3d::prelude::*;

#[kiss3d::main]
async fn main() {
    let webgpu = welt::grafik::vorbereiten().await;
    let einrichtung = welt::grafik::einrichtung("canvas", webgpu);
    let mut fenster = Window::new_with_setup("Welt: Minimalbeispiel", 800, 600, einrichtung).await;
    let mut kamera = OrbitCamera3d::default();
    let mut szene = SceneNode3d::empty();
    szene
        .add_light(Light::point(100.0))
        .set_position(Vec3::new(0.0, 2.0, -2.0));
    let mut wuerfel = szene.add_cube(1.0, 1.0, 1.0).set_color(RED);
    let drehung = Quat::from_axis_angle(Vec3::Y, 0.014);
    while fenster.render_3d(&mut szene, &mut kamera).await {
        wuerfel.rotate(drehung);
    }
}
