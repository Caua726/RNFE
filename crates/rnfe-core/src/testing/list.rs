//! Tabela única das ROMs de teste: usada por `tests/blargg.rs` e pelo binário `status`.
//!
//! Regra: `KnownFail` carrega o motivo conhecido. Quando a ROM passa a funcionar, o teste
//! falha pedindo a troca para `Pass` — assim a tabela nunca fica atrás do emulador.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    /// Resultado em `$6000` (assinatura `DE B0 61` em `$6001-$6003`), texto em `$6004`.
    Mem,
    /// Resultado escrito na tela: `PASSED`/`FAILED #n`, ou código `$NN` (`$01` = passou).
    Screen,
    /// A ROM imprime um CRC de 8 dígitos hex na tela; passa se for um dos esperados
    /// (alguns testes aceitam mais de um, conforme o alinhamento CPU/PPU).
    Crc(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub enum Expect {
    Pass,
    KnownFail(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct TestRom {
    /// Chave de agrupamento: nome do `#[test]` e da seção do STATUS.md.
    pub suite: &'static str,
    /// Caminho relativo a `test-roms/` (igual ao repo nes-test-roms).
    pub path: &'static str,
    pub area: &'static str,
    pub style: Style,
    pub expect: Expect,
    pub max_frames: u32,
}

const fn t(
    suite: &'static str,
    path: &'static str,
    area: &'static str,
    style: Style,
    expect: Expect,
    max_frames: u32,
) -> TestRom {
    TestRom { suite, path, area, style, expect, max_frames }
}

use Expect::{KnownFail as KF, Pass};
use Style::{Crc, Mem, Screen};

pub const NESTEST_ROM: &str = "other/nestest.nes";
pub const NESTEST_LOG: &str = "other/nestest.log";

pub const TESTS: &[TestRom] = &[
    // ---------------------------------------------------------------- CPU
    t("instr_test_v5", "instr_test-v5/rom_singles/01-basics.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/02-implied.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/03-immediate.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/04-zero_page.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/05-zp_xy.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/06-absolute.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/07-abs_xy.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/08-ind_x.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/09-ind_y.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/10-branches.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/11-stack.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/12-jmp_jsr.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/13-rts.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/14-rti.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/15-brk.nes", "cpu", Mem, Pass, 2000),
    t("instr_test_v5", "instr_test-v5/rom_singles/16-special.nes", "cpu", Mem, Pass, 2000),
    t("instr_timing", "instr_timing/rom_singles/1-instr_timing.nes", "cpu", Mem, Pass, 4000),
    t("instr_timing", "instr_timing/rom_singles/2-branch_timing.nes", "cpu", Mem, Pass, 2000),
    t("instr_misc", "instr_misc/rom_singles/01-abs_x_wrap.nes", "cpu", Mem, Pass, 1000),
    t("instr_misc", "instr_misc/rom_singles/02-branch_wrap.nes", "cpu", Mem, Pass, 1000),
    t("instr_misc", "instr_misc/rom_singles/03-dummy_reads.nes", "cpu", Mem, Pass, 1000),
    t("instr_misc", "instr_misc/rom_singles/04-dummy_reads_apu.nes", "cpu", Mem, Pass, 1000),
    t("cpu_interrupts", "cpu_interrupts_v2/rom_singles/1-cli_latency.nes", "cpu", Mem, Pass, 1000),
    t("cpu_interrupts", "cpu_interrupts_v2/rom_singles/2-nmi_and_brk.nes", "cpu", Mem, Pass, 1000),
    t("cpu_interrupts", "cpu_interrupts_v2/rom_singles/3-nmi_and_irq.nes", "cpu", Mem, Pass, 1000),
    t("cpu_interrupts", "cpu_interrupts_v2/rom_singles/4-irq_and_dma.nes", "cpu", Mem, Pass, 1000),
    t("cpu_interrupts", "cpu_interrupts_v2/rom_singles/5-branch_delays_irq.nes", "cpu", Mem, Pass, 1000),
    t("cpu_dummy", "cpu_dummy_reads/cpu_dummy_reads.nes", "cpu", Screen, Pass, 600),
    t("cpu_dummy", "cpu_dummy_writes/cpu_dummy_writes_oam.nes", "cpu", Mem, Pass, 2000),
    t(
        "cpu_dummy",
        "cpu_dummy_writes/cpu_dummy_writes_ppumem.nes",
        "cpu",
        Mem,
        KF("open bus da PPU (F2-05)"),
        2000,
    ),
    t(
        "cpu_exec_space",
        "cpu_exec_space/test_cpu_exec_space_ppuio.nes",
        "bus",
        Mem,
        KF("open bus (F2-05)"),
        2000,
    ),
    t("cpu_exec_space", "cpu_exec_space/test_cpu_exec_space_apu.nes", "bus", Mem, Pass, 2000),
    t("cpu_reset", "cpu_reset/ram_after_reset.nes", "cpu", Mem, Pass, 2000),
    t("cpu_reset", "cpu_reset/registers.nes", "cpu", Mem, Pass, 2000),
    t("branch_timing", "branch_timing_tests/1.Branch_Basics.nes", "cpu", Screen, Pass, 600),
    t("branch_timing", "branch_timing_tests/2.Backward_Branch.nes", "cpu", Screen, Pass, 600),
    t("branch_timing", "branch_timing_tests/3.Forward_Branch.nes", "cpu", Screen, Pass, 600),
    // ---------------------------------------------------------------- PPU
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/01-vbl_basics.nes", "ppu", Mem, Pass, 2000),
    t(
        "ppu_vbl_nmi",
        "ppu_vbl_nmi/rom_singles/02-vbl_set_time.nes",
        "ppu",
        Mem,
        KF("VBL fora do dot exato (F2-04)"),
        2000,
    ),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/03-vbl_clear_time.nes", "ppu", Mem, Pass, 2000),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/04-nmi_control.nes", "ppu", Mem, Pass, 2000),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/05-nmi_timing.nes", "ppu", Mem, Pass, 2000),
    t(
        "ppu_vbl_nmi",
        "ppu_vbl_nmi/rom_singles/06-suppression.nes",
        "ppu",
        Mem,
        KF("supressão de NMI (F2-04)"),
        2000,
    ),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/07-nmi_on_timing.nes", "ppu", Mem, Pass, 2000),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/08-nmi_off_timing.nes", "ppu", Mem, Pass, 2000),
    t("ppu_vbl_nmi", "ppu_vbl_nmi/rom_singles/09-even_odd_frames.nes", "ppu", Mem, Pass, 2000),
    t(
        "ppu_vbl_nmi",
        "ppu_vbl_nmi/rom_singles/10-even_odd_timing.nes",
        "ppu",
        Mem,
        KF("odd frame (F2-04)"),
        2000,
    ),
    t("vbl_nmi_timing", "vbl_nmi_timing/1.frame_basics.nes", "ppu", Screen, Pass, 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/2.vbl_timing.nes", "ppu", Screen, KF("VBL timing (F2-04)"), 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/3.even_odd_frames.nes", "ppu", Screen, KF("a verificar"), 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/4.vbl_clear_timing.nes", "ppu", Screen, Pass, 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/5.nmi_suppression.nes", "ppu", Screen, KF("supressão (F2-04)"), 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/6.nmi_disable.nes", "ppu", Screen, Pass, 1200),
    t("vbl_nmi_timing", "vbl_nmi_timing/7.nmi_timing.nes", "ppu", Screen, Pass, 1200),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/01.basics.nes", "ppu", Screen, Pass, 600),
    t(
        "sprite_hit",
        "sprite_hit_tests_2005.10.05/02.alignment.nes",
        "ppu",
        Screen,
        KF("Failed #4 (F2-06)"),
        600,
    ),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/03.corners.nes", "ppu", Screen, KF("a verificar"), 600),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/04.flip.nes", "ppu", Screen, KF("a verificar"), 600),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/05.left_clip.nes", "ppu", Screen, KF("a verificar"), 600),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/06.right_edge.nes", "ppu", Screen, KF("x=255 (F2-06)"), 600),
    t(
        "sprite_hit",
        "sprite_hit_tests_2005.10.05/07.screen_bottom.nes",
        "ppu",
        Screen,
        KF("a verificar"),
        600,
    ),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/08.double_height.nes", "ppu", Screen, Pass, 600),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/09.timing_basics.nes", "ppu", Screen, Pass, 600),
    t("sprite_hit", "sprite_hit_tests_2005.10.05/10.timing_order.nes", "ppu", Screen, Pass, 600),
    t(
        "sprite_hit",
        "sprite_hit_tests_2005.10.05/11.edge_timing.nes",
        "ppu",
        Screen,
        KF("x=255 (F2-06)"),
        600,
    ),
    t(
        "sprite_overflow",
        "sprite_overflow_tests/1.Basics.nes",
        "ppu",
        Screen,
        KF("overflow no 8º sprite, não no 9º (F2-06)"),
        600,
    ),
    t(
        "sprite_overflow",
        "sprite_overflow_tests/2.Details.nes",
        "ppu",
        Screen,
        KF("avaliação de sprites (F2-06)"),
        600,
    ),
    t(
        "sprite_overflow",
        "sprite_overflow_tests/3.Timing.nes",
        "ppu",
        Screen,
        KF("avaliação de sprites (F2-06)"),
        600,
    ),
    t(
        "sprite_overflow",
        "sprite_overflow_tests/4.Obscure.nes",
        "ppu",
        Screen,
        KF("avaliação de sprites (F2-06)"),
        600,
    ),
    t(
        "sprite_overflow",
        "sprite_overflow_tests/5.Emulator.nes",
        "ppu",
        Screen,
        KF("avaliação de sprites (F2-06)"),
        600,
    ),
    t("oam", "oam_read/oam_read.nes", "ppu", Mem, Pass, 2000),
    t("oam", "oam_stress/oam_stress.nes", "ppu", Mem, KF("a verificar"), 4000),
    t("ppu_misc", "ppu_open_bus/ppu_open_bus.nes", "ppu", Mem, KF("sem open bus (F2-05)"), 2000),
    t("ppu_misc", "ppu_read_buffer/test_ppu_read_buffer.nes", "ppu", Mem, KF("a verificar"), 4000),
    // ---------------------------------------------------------------- APU
    t("apu_test", "apu_test/rom_singles/1-len_ctr.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/2-len_table.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/3-irq_flag.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/4-jitter.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/5-len_timing.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/6-irq_flag_timing.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/7-dmc_basics.nes", "apu", Mem, Pass, 1000),
    t("apu_test", "apu_test/rom_singles/8-dmc_rates.nes", "apu", Mem, Pass, 1000),
    t("apu_2005", "blargg_apu_2005.07.30/01.len_ctr.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/02.len_table.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/03.irq_flag.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/04.clock_jitter.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/05.len_timing_mode0.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/06.len_timing_mode1.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/07.irq_flag_timing.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/08.irq_timing.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/09.reset_timing.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/10.len_halt_timing.nes", "apu", Screen, Pass, 600),
    t("apu_2005", "blargg_apu_2005.07.30/11.len_reload_timing.nes", "apu", Screen, Pass, 600),
    t("apu_reset", "apu_reset/4015_cleared.nes", "apu", Mem, Pass, 1000),
    t("apu_reset", "apu_reset/4017_timing.nes", "apu", Mem, Pass, 1000),
    t("apu_reset", "apu_reset/4017_written.nes", "apu", Mem, Pass, 1000),
    t("apu_reset", "apu_reset/irq_flag_cleared.nes", "apu", Mem, Pass, 1000),
    t("apu_reset", "apu_reset/len_ctrs_enabled.nes", "apu", Mem, Pass, 1000),
    t("apu_reset", "apu_reset/works_immediately.nes", "apu", Mem, Pass, 1000),
    t("dmc", "dmc_dma_during_read4/dma_2007_read.nes", "apu", Crc(&["159A7A8F", "5E3DF9C4"]), Pass, 600),
    t("dmc", "dmc_dma_during_read4/dma_2007_write.nes", "apu", Crc(&["28F53CA4"]), Pass, 600),
    t("dmc", "dmc_dma_during_read4/dma_4016_read.nes", "apu", Crc(&["F0AB808C"]), Pass, 600),
    t(
        "dmc",
        "dmc_dma_during_read4/double_2007_read.nes",
        "apu",
        Crc(&["85CFD627", "F018C287", "440EF923", "E52F41A5"]),
        KF("leituras de $2007 em ciclos consecutivos: a PPU ignora a 2ª (quirk não modelado)"),
        600,
    ),
    t("dmc", "dmc_dma_during_read4/read_write_2007.nes", "apu", Crc(&["0F877C4B"]), Pass, 600),
    // `dmc_tests/*.nes` (buffer_retained, latency, status, status_irq) não têm veredito legível:
    // tocam uma amostra e ficam num tom (`JMP *`); só avaliáveis de ouvido. Fora da tabela.
    // ---------------------------------------------------------------- mappers
    t(
        "mmc3",
        "mmc3_test_2/rom_singles/1-clocking.nes",
        "mapper",
        Mem,
        KF("IRQ por pulso fixo, não por A12 (F3-02)"),
        1000,
    ),
    t("mmc3", "mmc3_test_2/rom_singles/2-details.nes", "mapper", Mem, KF("A12 (F3-02)"), 1000),
    t("mmc3", "mmc3_test_2/rom_singles/3-A12_clocking.nes", "mapper", Mem, KF("A12 (F3-02)"), 1000),
    t("mmc3", "mmc3_test_2/rom_singles/4-scanline_timing.nes", "mapper", Mem, KF("A12 (F3-02)"), 1000),
    t("mmc3", "mmc3_test_2/rom_singles/5-MMC3.nes", "mapper", Mem, KF("A12 (F3-02)"), 1000),
    t("mmc3", "mmc3_test_2/rom_singles/6-MMC3_alt.nes", "mapper", Mem, KF("variante A (F3-02)"), 1000),
    t("mmc3_irq", "mmc3_irq_tests/1.Clocking.nes", "mapper", Screen, KF("A12 (F3-02)"), 600),
    t("mmc3_irq", "mmc3_irq_tests/2.Details.nes", "mapper", Screen, KF("A12 (F3-02)"), 600),
    t("mmc3_irq", "mmc3_irq_tests/3.A12_clocking.nes", "mapper", Screen, KF("A12 (F3-02)"), 600),
    t("mmc3_irq", "mmc3_irq_tests/4.Scanline_timing.nes", "mapper", Screen, Pass, 600),
    t("mmc3_irq", "mmc3_irq_tests/5.MMC3_rev_A.nes", "mapper", Screen, KF("variante A (F3-02)"), 600),
    t("mmc3_irq", "mmc3_irq_tests/6.MMC3_rev_B.nes", "mapper", Screen, KF("A12 (F3-02)"), 600),
];
