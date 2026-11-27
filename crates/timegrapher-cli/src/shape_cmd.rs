//! `timegrapher shape`: the three sounds of each side's beat.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use timegrapher_core::analysis::{self, AnalysisConfig, Summary};
use timegrapher_core::library::{fixed_text, signed_text};
use timegrapher_core::load;
use timegrapher_core::measure::Scope;
use timegrapher_core::shape::{Shape, ShapeReport, SideReport};

pub struct Options<'a> {
    /// What `analyze` reads the recording with: the beats, their sides
    /// and the amplitude are its own (#2104).
    pub cfg: AnalysisConfig,
    pub json: bool,
    pub templates: Option<&'a Path>,
}

pub fn run(file: &Path, o: &Options) -> Result<(), String> {
    let audio = load(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let a = analysis::analyze(&audio, &o.cfg);
    let r = analysis::shape_report(&audio, &o.cfg, &a);
    if let Some(p) = o.templates {
        write_templates(p, &r, audio.sample_rate as f64)
            .map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let s = &a.summary;
    if o.json {
        let settings = serde_json::json!({
            "bph": s.bph,
            "lift_deg": o.cfg.amplitude.lift_deg,
            "notch_hz": o.cfg.envelope.notch_hz,
            "highpass_hz": o.cfg.envelope.highpass_hz,
        });
        crate::output::print(
            "shape",
            crate::output::input(&[file.to_path_buf()], settings),
            &r,
        )?;
    } else if s.shape.is_none() {
        // No shape where `analyze` reads none, and no table: `analyze`'s
        // own line for it, no regular beat found or no verdict yet on a
        // recording under 10 s (#2371).
        let line = crate::no_watch_line(s).or_else(|| crate::regularity_line(s));
        if let Some(line) = line {
            println!("{line}");
        }
    } else {
        print_report(&r, s);
    }
    Ok(())
}

fn write_templates(p: &Path, r: &ShapeReport, fs: f64) -> std::io::Result<()> {
    let mut w = BufWriter::new(File::create(p)?);
    writeln!(w, "time_ms,even,odd")?;
    for (k, (e, o)) in r.template_even.iter().zip(&r.template_odd).enumerate() {
        let t = r.template_start_ms + k as f64 / fs * 1000.0;
        writeln!(w, "{t:.4},{e:.6e},{o:.6e}")?;
    }
    w.flush()
}

fn opt(v: Option<f64>, digits: usize) -> String {
    v.filter(|x| x.is_finite())
        .map_or("-".into(), |x| fixed_text(x, digits))
}

/// The shape table. Each side's amplitude is `analyze`'s, from `s`, with
/// the caveats it carries there: it is read over the amplitude windows,
/// not from these templates, so the whole-recording columns have none
/// (#2104).
fn print_report(r: &ShapeReport, s: &Summary) {
    println!(
        "Beat rate    {} bph; {} windows of 2 s; times from the unlock edge",
        s.bph,
        r.windows.len()
    );
    let cols: [(&str, Option<&Shape>); 4] = [
        ("Tick", Some(&r.even.windows)),
        ("Tock", Some(&r.odd.windows)),
        ("Tick all", r.even.whole.as_ref()),
        ("Tock all", r.odd.whole.as_ref()),
    ];
    print!("{:<24}", "");
    for (name, _) in &cols {
        print!("{name:>10}");
    }
    println!();
    print!("{:<24}", "amplitude, deg");
    for v in [s.amplitude_even_deg, s.amplitude_odd_deg, None, None] {
        print!("{:>10}", opt(v, 0));
    }
    println!();
    // The amplitude carries the caveats `analyze` gives it, in core's
    // words (a co-axial's estimate among them).
    if s.amplitude_even_deg.is_some() || s.amplitude_odd_deg.is_some() {
        for c in s
            .caveats
            .iter()
            .filter(|c| c.scopes().contains(&Scope::Amplitude))
        {
            if let Some(w) = c.short(s.lift_deg, s.beats_found) {
                println!("  {w}");
            }
        }
    }
    type Row<'a> = (&'a str, &'a dyn Fn(&Shape) -> Option<f64>, usize);
    let rows: [Row; 21] = [
        ("unlock to drop, ms", &|s| Some(s.unlock_to_drop_ms), 2),
        ("sound 1 rise, ms", &|s| s.t1_ms, 2),
        ("sound 2 rise, ms", &|s| s.t2_ms, 2),
        ("sound 3 rise, ms", &|s| Some(s.t3_ms), 2),
        ("sound 1 peak, ms", &|s| s.peak1_ms, 2),
        ("sound 2 peak, ms", &|s| s.peak2_ms, 2),
        ("sound 3 peak, ms", &|s| Some(s.peak3_ms), 2),
        ("interval 1-2, ms", &|s| s.i12_ms, 2),
        ("interval 2-3, ms", &|s| s.i23_ms, 2),
        ("interval 1-3, ms", &|s| s.i13_ms, 2),
        ("level 1:3", &|s| s.ratio13, 3),
        ("level 2:3", &|s| s.ratio23, 3),
        ("valley 1-2", &|s| s.valley12, 3),
        ("valley 2-3", &|s| s.valley23, 3),
        ("fill 1-2", &|s| s.fill12, 3),
        ("fill 2-3", &|s| s.fill23, 3),
        ("tail 3-15 ms after 3", &|s| Some(s.tail_ratio), 3),
        ("noise between beats", &|s| s.noise_ratio, 2),
        ("rises from 1 to 3", &|s| Some(s.rises as f64), 0),
        (
            "extra events before 1",
            &|s| Some(s.extra_pre.len() as f64),
            0,
        ),
        (
            "extra events after 3",
            &|s| Some(s.extra_post.len() as f64),
            0,
        ),
    ];
    for (name, f, digits) in rows {
        print!("{name:<24}");
        for (k, (_, s)) in cols.iter().enumerate() {
            // Extra events are counted on the whole-recording templates only.
            let v = if k < 2 && name.starts_with("extra") {
                None
            } else {
                s.and_then(f)
            };
            print!("{:>10}", opt(v, digits));
        }
        println!();
    }
    for (name, s) in [("Tick", &r.even), ("Tock", &r.odd)] {
        print_side(name, s);
    }
}

fn print_side(name: &str, s: &SideReport) {
    println!(
        "{name:<5} sound 1 found in {}/{} windows, sound 2 in {}, extra events before 1 in {}, after 3 in {}",
        s.windows_with_1,
        s.windows_measured,
        s.windows_with_2,
        s.windows_with_extra_pre,
        s.windows_with_extra_post
    );
    if let Some(p) = &s.spread {
        println!(
            "      single beats: interval 1-3 {} ms, spread {} us (robust SD, {} beats)",
            fixed_text(p.i13_ms, 2),
            fixed_text(p.i13_sd_us, 0),
            p.beats
        );
    }
    if let Some(w) = &s.whole {
        for (when, list) in [("before 1", &w.extra_pre), ("after 3", &w.extra_post)] {
            for e in list {
                println!(
                    "      extra event {when} at {} ms, level {} of the drop",
                    signed_text(e.t_ms, 2),
                    fixed_text(e.level, 3)
                );
            }
        }
    }
}
