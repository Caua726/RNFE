//! Roda as ROMs de teste da comunidade (blargg e outros) contra a tabela em
//! `rnfe_core::testing::list`. Um `#[test]` por suíte.
//!
//! Regras:
//! - `Expect::Pass` que falha → erro.
//! - `Expect::KnownFail` que **passa** → erro ("agora PASSA — troque para Pass"), para que a
//!   tabela nunca fique atrás do emulador.
//! - ROM ausente → SKIP (erro só com `RNFE_REQUIRE_ROMS=1`, como no CI).
//! - `RNFE_TEST_FILTER=substring` restringe pelos paths.

use rnfe_core::testing::{Expect, Outcome, TESTS, run};

fn suite(name: &str) {
    let require = std::env::var_os("RNFE_REQUIRE_ROMS").is_some();
    let filter = std::env::var("RNFE_TEST_FILTER").unwrap_or_default();
    let mut errors = Vec::new();
    let mut ran = 0;
    for t in TESTS.iter().filter(|t| t.suite == name && t.path.contains(&filter)) {
        let out = run(t);
        let verdict: Result<&str, String> = match (&t.expect, &out) {
            (_, Outcome::Skip(m)) if !require => {
                eprintln!("SKIP       {}: {m}", t.path);
                continue;
            }
            (_, Outcome::Skip(m)) => Err(format!("ROM obrigatória ausente: {m}")),
            (Expect::Pass, Outcome::Pass) => Ok("ok"),
            (Expect::Pass, Outcome::Fail(m) | Outcome::Timeout(m)) => Err(format!("esperava PASS: {m}")),
            (Expect::KnownFail(_), Outcome::Fail(_) | Outcome::Timeout(_)) => Ok("known-fail"),
            (Expect::KnownFail(why), Outcome::Pass) => {
                Err(format!("agora PASSA (era KnownFail: {why}) — troque para Pass em src/testing/list.rs"))
            }
        };
        ran += 1;
        match verdict {
            Ok(tag) => eprintln!("{tag:<10} {}", t.path),
            Err(e) => errors.push(format!("{}: {e}", t.path)),
        }
    }
    assert!(
        errors.is_empty(),
        "suíte {name}: {} problema(s) em {ran} ROMs\n  {}",
        errors.len(),
        errors.join("\n  ")
    );
}

macro_rules! suites {
    ($($id:ident),* $(,)?) => {
        $( #[test] fn $id() { suite(stringify!($id)); } )*
        const SUITES: &[&str] = &[$(stringify!($id)),*];
    };
}

suites!(
    instr_test_v5,
    instr_timing,
    instr_misc,
    cpu_interrupts,
    cpu_dummy,
    cpu_exec_space,
    cpu_reset,
    branch_timing,
    ppu_vbl_nmi,
    vbl_nmi_timing,
    sprite_hit,
    sprite_overflow,
    oam,
    ppu_misc,
    apu_test,
    apu_2005,
    apu_reset,
    dmc,
    mmc3,
    mmc3_irq,
);

/// Toda entrada da tabela precisa pertencer a uma suíte com `#[test]`.
#[test]
fn every_table_entry_has_a_suite() {
    for t in TESTS {
        assert!(SUITES.contains(&t.suite), "suíte '{}' (ROM {}) não tem #[test]", t.suite, t.path);
    }
}
