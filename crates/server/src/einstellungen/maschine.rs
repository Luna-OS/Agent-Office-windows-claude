//! Der Rechner des Office: CPU und Speicher für den Wandmonitor und die Warnung vor dem Einstellen.

use std::collections::VecDeque;
use std::sync::Mutex;

use protokoll::einstellungen::Maschine;
use sysinfo::System;

const VERLAUF: usize = 60;

pub struct Rechner {
    sys: Mutex<System>,
    verlauf: Mutex<VecDeque<[f32; 2]>>,
}

impl Default for Rechner {
    fn default() -> Self {
        Self { sys: Mutex::new(System::new()), verlauf: Mutex::default() }
    }
}

impl Rechner {
    /// Misst neu (alle paar Sekunden aufrufen).
    pub fn messen(&self) {
        let mut s = self.sys.lock().unwrap();
        s.refresh_cpu_usage();
        s.refresh_memory();
        let cpu = s.global_cpu_usage();
        let mem = if s.total_memory() > 0 { s.used_memory() as f32 * 100.0 / s.total_memory() as f32 } else { 0.0 };
        let mut v = self.verlauf.lock().unwrap();
        v.push_back([cpu.round(), mem.round()]);
        while v.len() > VERLAUF {
            v.pop_front();
        }
    }

    pub fn stand(&self, arbeiter: u32, limit: Option<u32>, obergrenze: Option<u32>) -> Maschine {
        let s = self.sys.lock().unwrap();
        let verlauf: Vec<[f32; 2]> = self.verlauf.lock().unwrap().iter().copied().collect();
        let [cpu, mem] = verlauf.last().copied().unwrap_or([0.0, 0.0]);
        let druck = if mem >= 90.0 {
            Some(format!("Speicher zu {mem:.0} % belegt"))
        } else if cpu >= 90.0 {
            Some(format!("Prozessor zu {cpu:.0} % ausgelastet"))
        } else {
            None
        };
        Maschine {
            cpu,
            kerne: s.cpus().len().max(1) as u32,
            speicher_belegt: s.used_memory(),
            speicher_gesamt: s.total_memory(),
            verlauf,
            druck,
            arbeiter,
            limit,
            obergrenze,
        }
    }
}
