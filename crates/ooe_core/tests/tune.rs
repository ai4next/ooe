//! `cargo test -p ooe_core --test tune -- --nocapture --ignored` prints every
//! reference and starter's measurements, for setting goals.
use ooe_core::content::tasks;
use ooe_core::grade::grade;

#[test]
#[ignore]
fn print_measures() {
    for t in tasks() {
        for (name, src) in [("ref", t.reference), ("start", t.starter)] {
            match grade(t, src) {
                Ok(r) => {
                    let whys: Vec<String> = r
                        .runs
                        .iter()
                        .map(|c| format!("{:?}@{:.2}", c.why, c.end()))
                        .collect();
                    println!(
                        "{:14} {:5} {:?} stars={} {:?}\n    {:?}",
                        t.id,
                        name,
                        r.verdict,
                        r.stars(),
                        r.measure,
                        whys
                    );
                }
                Err(e) => println!("{:14} {:5} ERR {e:?}", t.id, name),
            }
        }
    }
}

/// `TUNE_ID=<id> TUNE_SRC='...'` grades one program.
#[test]
#[ignore]
fn try_one() {
    let (Ok(id), Ok(src)) = (std::env::var("TUNE_ID"), std::env::var("TUNE_SRC")) else {
        return;
    };
    let t = ooe_core::content::task(&id).unwrap();
    for src in src.split("||") {
        let r = grade(t, &src.replace('/', "\n")).unwrap();
        let ends: Vec<String> = r
            .runs
            .iter()
            .map(|c| {
                format!(
                    "{:?}@{:.2} os={:.3} t={:.2}",
                    c.why,
                    c.end(),
                    c.measure.overshoot,
                    c.measure.time
                )
            })
            .collect();
        let m = r.measure;
        if std::env::var("TUNE_BRIEF").is_ok() {
            println!(
                "{src:60} {} t={:.2} os={:.3} err={:.3} eff={:.1} {:?}",
                r.stars(),
                m.time,
                m.overshoot,
                m.error,
                m.effort,
                r.verdict
            );
        } else {
            println!(
                "{src}\n  {:?} stars={} {:?}\n  {ends:?}",
                r.verdict,
                r.stars(),
                r.measure
            );
        }
    }
}
