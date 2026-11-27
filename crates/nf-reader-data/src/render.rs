//! Rasterise with tools already on the host: rsvg-convert (SVG to PNG and PDF) and ImageMagick (grey, scan effects).

use crate::layout::Page;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[derive(Clone, Debug)]
pub struct Fx {
    pub skew: f64,
    pub blur: f64,
    pub noise: f64,
    pub lo: u32,
    pub hi: u32,
    pub jpeg: u32,
    pub seed: u64,
}

pub fn svg(p: &Page, skew: f64, paper: &str) -> String {
    svg_of(p, &p.body, skew, paper)
}

/// The page with its text elements only (no lines, no geometry): the box test renders this.
pub fn svg_text_only(p: &Page, skew: f64, paper: &str) -> String {
    svg_of(p, &p.text_body, skew, paper)
}

fn svg_of(p: &Page, body: &str, skew: f64, paper: &str) -> String {
    let (w, h) = (p.w, p.h);
    let g = if skew != 0.0 {
        format!(
            "<g transform=\"rotate({skew:.3} {:.1} {:.1})\">",
            w / 2.0,
            h / 2.0
        )
    } else {
        "<g>".to_string()
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"{paper}\"/>\n{g}\n{}</g>\n</svg>\n",
        body
    )
}

fn spawn_rsvg(args: &[&str], svg: &str, out: Stdio) -> Result<Child, String> {
    let mut c = Command::new("rsvg-convert")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("rsvg-convert: {e}"))?;
    let mut si = c.stdin.take().ok_or("no stdin")?;
    let s = svg.to_string();
    std::thread::spawn(move || {
        let _ = si.write_all(s.as_bytes());
    });
    Ok(c)
}

fn finish(mut c: Child, what: &str) -> Result<(), String> {
    let st = c.wait().map_err(|e| format!("{what}: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("{what} failed: {st}"))
    }
}

pub fn render_pdf(svg: &str, out: &Path) -> Result<(), String> {
    let f = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let c = spawn_rsvg(&["-f", "pdf"], svg, Stdio::from(f))?;
    finish(c, "rsvg-convert pdf")
}

pub fn render_png(svg: &str, w: u32, h: u32, out: &Path, fx: Option<&Fx>) -> Result<(), String> {
    let (ws, hs) = (w.to_string(), h.to_string());
    let mut r = spawn_rsvg(&["-w", &ws, "-h", &hs, "-f", "png"], svg, Stdio::piped())?;
    let rout = r.stdout.take().ok_or("no stdout")?;
    let outp = format!("png:{}", out.display());
    let mut cmds: Vec<Child> = vec![];
    match fx {
        None => {
            let c = Command::new("convert")
                .args([
                    "png:-",
                    "-colorspace",
                    "Gray",
                    "-depth",
                    "8",
                    "-define",
                    "png:color-type=0",
                    &outp,
                ])
                .stdin(Stdio::from(rout))
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("convert: {e}"))?;
            cmds.push(c);
        }
        Some(f) => {
            let mut a: Vec<String> = vec!["png:-".into(), "-colorspace".into(), "Gray".into()];
            if f.blur > 0.0 {
                a.extend(["-blur".into(), format!("0x{}", f.blur)]);
            }
            a.extend(["+level".into(), format!("{}%,{}%", f.lo, f.hi)]);
            a.extend([
                "-seed".into(),
                f.seed.to_string(),
                "-attenuate".into(),
                format!("{}", f.noise),
                "+noise".into(),
                "Gaussian".into(),
            ]);
            a.extend(["-quality".into(), f.jpeg.to_string(), "jpg:-".into()]);
            let mut c1 = Command::new("convert")
                .args(&a)
                .stdin(Stdio::from(rout))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("convert: {e}"))?;
            let o1 = c1.stdout.take().ok_or("no stdout")?;
            let c2 = Command::new("convert")
                .args([
                    "jpg:-",
                    "-colorspace",
                    "Gray",
                    "-depth",
                    "8",
                    "-define",
                    "png:color-type=0",
                    &outp,
                ])
                .stdin(Stdio::from(o1))
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("convert: {e}"))?;
            cmds.push(c1);
            cmds.push(c2);
        }
    }
    let mut res = finish(r, "rsvg-convert png");
    for c in cmds {
        let e = finish(c, "convert");
        if res.is_ok() {
            res = e;
        }
    }
    res
}

/// Decode a PNG to raw 8-bit grey (for tests and the contact tool).
pub fn read_gray(path: &Path) -> Result<Vec<u8>, String> {
    let o = Command::new("convert")
        .arg(path)
        .args(["-colorspace", "Gray", "-depth", "8", "gray:-"])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(o.stdout)
    } else {
        Err("convert read failed".into())
    }
}
