//! Gera `docs/STATUS.md`: roda todas as ROMs da tabela e o nestest, imprime markdown em stdout.
//!
//! `cargo run -p rnfe-core --release --bin status > docs/STATUS.md`

use rnfe_core::testing::{nestest, run, Expect, Outcome, Style, TestRom, TESTS};
use std::fmt::Write;

fn main() {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 6);
    let results: Vec<(usize, Outcome)> = std::thread::scope(|s| {
        let chunks: Vec<Vec<usize>> = (0..threads).map(|k| (k..TESTS.len()).step_by(threads).collect()).collect();
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|idx| s.spawn(move || idx.into_iter().map(|i| (i, run(&TESTS[i]))).collect::<Vec<_>>()))
            .collect();
        let mut all: Vec<(usize, Outcome)> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
        all.sort_by_key(|(i, _)| *i);
        all
    });

    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "?".into());

    let mut out = String::new();
    let _ = writeln!(out, "# Status do RNFE");
    let _ = writeln!(out);
    let _ = writeln!(out, "Gerado por `cargo run -p rnfe-core --release --bin status` · commit `{commit}`.");
    let _ = writeln!(out, "Não edite à mão: a fonte é `crates/rnfe-core/src/testing/list.rs` + `cargo test`.");
    let _ = writeln!(out);

    match nestest::compare() {
        Ok(r) => {
            let _ = writeln!(
                out,
                "**nestest:** {}/{} linhas idênticas ao log (registradores e ciclos){}",
                r.matched,
                nestest::TOTAL_LINES,
                r.first_bad
                    .map(|(i, _)| format!("; primeira divergência na linha {}", i + 1))
                    .unwrap_or_default()
            );
        }
        Err(e) => {
            let _ = writeln!(out, "**nestest:** não rodou — {e}");
        }
    }
    let _ = writeln!(out);

    let (mut pass, mut fail_expected, mut surprise, mut regress, mut skip) = (0, 0, 0, 0, 0);
    let mut area = "";
    for (i, outcome) in &results {
        let t: &TestRom = &TESTS[*i];
        if t.area != area {
            area = t.area;
            let _ = writeln!(out, "## {area}");
            let _ = writeln!(out);
            let _ = writeln!(out, "| ROM | Estilo | Esperado | Resultado | Detalhe |");
            let _ = writeln!(out, "|---|---|---|---|---|");
        }
        let style = match t.style {
            Style::Mem => "$6000",
            Style::Screen => "tela",
        };
        let (expect, why) = match t.expect {
            Expect::Pass => ("Pass", ""),
            Expect::KnownFail(w) => ("KnownFail", w),
        };
        let (mark, detail) = match (&t.expect, outcome) {
            (_, Outcome::Skip(m)) => {
                skip += 1;
                ("⏭", m.clone())
            }
            (Expect::Pass, Outcome::Pass) => {
                pass += 1;
                ("✅", String::new())
            }
            (Expect::KnownFail(_), Outcome::Pass) => {
                surprise += 1;
                ("⚠️ passa (tabela desatualizada)", why.to_string())
            }
            (Expect::Pass, Outcome::Fail(m) | Outcome::Timeout(m)) => {
                regress += 1;
                ("🔴 regressão", m.clone())
            }
            (Expect::KnownFail(_), Outcome::Fail(m) | Outcome::Timeout(m)) => {
                fail_expected += 1;
                ("❌", format!("{why} — {m}"))
            }
        };
        let _ = writeln!(
            out,
            "| `{}` | {style} | {expect} | {mark} | {} |",
            t.path,
            detail.replace('|', "/").replace('\n', " ")
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "## Resumo: {pass} ✅ · {fail_expected} ❌ esperados · {surprise} ⚠️ · {regress} 🔴 · {skip} ⏭ (de {} ROMs)",
        TESTS.len()
    );
    print!("{out}");
    if surprise + regress > 0 {
        eprintln!("atenção: {surprise} ROM(s) passam sem a tabela saber e {regress} regrediram");
        std::process::exit(1);
    }
}
