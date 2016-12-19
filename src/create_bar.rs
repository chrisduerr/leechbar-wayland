use rand;
use tempfile;
use std::fs::File;
use std::{thread, time};
use std::io::{self, Write};
use std::sync::mpsc::Sender;
use byteorder::{WriteBytesExt, NativeEndian};
use rand::distributions::{IndependentSample, Range};

pub fn start_bar_creator(bar_img_out: &Sender<File>) {
    loop {
        bar_img_out.send(get_tmp().unwrap()).unwrap();
        thread::sleep(time::Duration::from_millis(1000));
    }
}

fn get_tmp() -> Result<File, io::Error> {
    let mut tmp = tempfile::tempfile()?;
    let between = Range::new(0, 0xFF);
    let mut rng = rand::thread_rng();
    for _ in 0..20_000 {
        let r: u32 = between.ind_sample(&mut rng);
        let g: u32 = between.ind_sample(&mut rng);
        let b: u32 = between.ind_sample(&mut rng);
        let _ = tmp.write_u32::<NativeEndian>((0xFF << 24) + (r << 16) + (g << 8) + b);
    }
    let _ = tmp.flush();
    Ok(tmp)
}
