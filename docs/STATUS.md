# Status do RNFE

Gerado por `cargo run -p rnfe-core --release --bin status` · commit `2b4e779`.
Não edite à mão: a fonte é `crates/rnfe-core/src/testing/list.rs` + `cargo test`.

**nestest:** 8991/8991 linhas idênticas ao log (registradores e ciclos)

## cpu

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `instr_test-v5/rom_singles/01-basics.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/02-implied.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/03-immediate.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/04-zero_page.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/05-zp_xy.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/06-absolute.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/07-abs_xy.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/08-ind_x.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/09-ind_y.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/10-branches.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/11-stack.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/12-jmp_jsr.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/13-rts.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/14-rti.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/15-brk.nes` | $6000 | Pass | ✅ |  |
| `instr_test-v5/rom_singles/16-special.nes` | $6000 | Pass | ✅ |  |
| `instr_timing/rom_singles/1-instr_timing.nes` | $6000 | Pass | ✅ |  |
| `instr_timing/rom_singles/2-branch_timing.nes` | $6000 | Pass | ✅ |  |
| `instr_misc/rom_singles/01-abs_x_wrap.nes` | $6000 | Pass | ✅ |  |
| `instr_misc/rom_singles/02-branch_wrap.nes` | $6000 | Pass | ✅ |  |
| `instr_misc/rom_singles/03-dummy_reads.nes` | $6000 | Pass | ✅ |  |
| `instr_misc/rom_singles/04-dummy_reads_apu.nes` | $6000 | Pass | ✅ |  |
| `cpu_interrupts_v2/rom_singles/1-cli_latency.nes` | $6000 | Pass | ✅ |  |
| `cpu_interrupts_v2/rom_singles/2-nmi_and_brk.nes` | $6000 | Pass | ✅ |  |
| `cpu_interrupts_v2/rom_singles/3-nmi_and_irq.nes` | $6000 | Pass | ✅ |  |
| `cpu_interrupts_v2/rom_singles/4-irq_and_dma.nes` | $6000 | Pass | ✅ |  |
| `cpu_interrupts_v2/rom_singles/5-branch_delays_irq.nes` | $6000 | Pass | ✅ |  |
| `cpu_dummy_reads/cpu_dummy_reads.nes` | tela | Pass | ✅ |  |
| `cpu_dummy_writes/cpu_dummy_writes_oam.nes` | $6000 | Pass | ✅ |  |
| `cpu_dummy_writes/cpu_dummy_writes_ppumem.nes` | $6000 | Pass | ✅ |  |
## bus

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `cpu_exec_space/test_cpu_exec_space_ppuio.nes` | $6000 | Pass | ✅ |  |
| `cpu_exec_space/test_cpu_exec_space_apu.nes` | $6000 | Pass | ✅ |  |
## cpu

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `cpu_reset/ram_after_reset.nes` | $6000 | Pass | ✅ |  |
| `cpu_reset/registers.nes` | $6000 | Pass | ✅ |  |
| `branch_timing_tests/1.Branch_Basics.nes` | tela | Pass | ✅ |  |
| `branch_timing_tests/2.Backward_Branch.nes` | tela | Pass | ✅ |  |
| `branch_timing_tests/3.Forward_Branch.nes` | tela | Pass | ✅ |  |
## ppu

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `ppu_vbl_nmi/rom_singles/01-vbl_basics.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/02-vbl_set_time.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/03-vbl_clear_time.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/04-nmi_control.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/05-nmi_timing.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/06-suppression.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/07-nmi_on_timing.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/08-nmi_off_timing.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/09-even_odd_frames.nes` | $6000 | Pass | ✅ |  |
| `ppu_vbl_nmi/rom_singles/10-even_odd_timing.nes` | $6000 | Pass | ✅ |  |
| `vbl_nmi_timing/1.frame_basics.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/2.vbl_timing.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/3.even_odd_frames.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/4.vbl_clear_timing.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/5.nmi_suppression.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/6.nmi_disable.nes` | tela | Pass | ✅ |  |
| `vbl_nmi_timing/7.nmi_timing.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/01.basics.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/02.alignment.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/03.corners.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/04.flip.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/05.left_clip.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/06.right_edge.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/07.screen_bottom.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/08.double_height.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/09.timing_basics.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/10.timing_order.nes` | tela | Pass | ✅ |  |
| `sprite_hit_tests_2005.10.05/11.edge_timing.nes` | tela | Pass | ✅ |  |
| `sprite_overflow_tests/1.Basics.nes` | tela | Pass | ✅ |  |
| `sprite_overflow_tests/2.Details.nes` | tela | Pass | ✅ |  |
| `sprite_overflow_tests/3.Timing.nes` | tela | Pass | ✅ |  |
| `sprite_overflow_tests/4.Obscure.nes` | tela | Pass | ✅ |  |
| `sprite_overflow_tests/5.Emulator.nes` | tela | Pass | ✅ |  |
| `oam_read/oam_read.nes` | $6000 | Pass | ✅ |  |
| `oam_stress/oam_stress.nes` | $6000 | Pass | ✅ |  |
| `ppu_open_bus/ppu_open_bus.nes` | $6000 | Pass | ✅ |  |
| `ppu_read_buffer/test_ppu_read_buffer.nes` | $6000 | Pass | ✅ |  |
## apu

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `apu_test/rom_singles/1-len_ctr.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/2-len_table.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/3-irq_flag.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/4-jitter.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/5-len_timing.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/6-irq_flag_timing.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/7-dmc_basics.nes` | $6000 | Pass | ✅ |  |
| `apu_test/rom_singles/8-dmc_rates.nes` | $6000 | Pass | ✅ |  |
| `blargg_apu_2005.07.30/01.len_ctr.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/02.len_table.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/03.irq_flag.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/04.clock_jitter.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/05.len_timing_mode0.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/06.len_timing_mode1.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/07.irq_flag_timing.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/08.irq_timing.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/09.reset_timing.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/10.len_halt_timing.nes` | tela | Pass | ✅ |  |
| `blargg_apu_2005.07.30/11.len_reload_timing.nes` | tela | Pass | ✅ |  |
| `apu_reset/4015_cleared.nes` | $6000 | Pass | ✅ |  |
| `apu_reset/4017_timing.nes` | $6000 | Pass | ✅ |  |
| `apu_reset/4017_written.nes` | $6000 | Pass | ✅ |  |
| `apu_reset/irq_flag_cleared.nes` | $6000 | Pass | ✅ |  |
| `apu_reset/len_ctrs_enabled.nes` | $6000 | Pass | ✅ |  |
| `apu_reset/works_immediately.nes` | $6000 | Pass | ✅ |  |
| `dmc_dma_during_read4/dma_2007_read.nes` | CRC | Pass | ✅ |  |
| `dmc_dma_during_read4/dma_2007_write.nes` | CRC | Pass | ✅ |  |
| `dmc_dma_during_read4/dma_4016_read.nes` | CRC | Pass | ✅ |  |
| `dmc_dma_during_read4/double_2007_read.nes` | CRC | KnownFail | ❌ | leituras de $2007 em ciclos consecutivos: a PPU ignora a 2ª (quirk não modelado) — CRC D84F6815, esperado 85CFD627/F018C287/440EF923/E52F41A5 |
| `dmc_dma_during_read4/read_write_2007.nes` | CRC | Pass | ✅ |  |
## mapper

| ROM | Estilo | Esperado | Resultado | Detalhe |
|---|---|---|---|---|
| `mmc3_test_2/rom_singles/1-clocking.nes` | $6000 | Pass | ✅ |  |
| `mmc3_test_2/rom_singles/2-details.nes` | $6000 | Pass | ✅ |  |
| `mmc3_test_2/rom_singles/3-A12_clocking.nes` | $6000 | Pass | ✅ |  |
| `mmc3_test_2/rom_singles/4-scanline_timing.nes` | $6000 | Pass | ✅ |  |
| `mmc3_test_2/rom_singles/5-MMC3.nes` | $6000 | Pass | ✅ |  |
| `mmc3_test_2/rom_singles/6-MMC3_alt.nes` | $6000 | Pass | ✅ |  |
| `mmc3_irq_tests/1.Clocking.nes` | tela | Pass | ✅ |  |
| `mmc3_irq_tests/2.Details.nes` | tela | Pass | ✅ |  |
| `mmc3_irq_tests/3.A12_clocking.nes` | tela | Pass | ✅ |  |
| `mmc3_irq_tests/4.Scanline_timing.nes` | tela | Pass | ✅ |  |
| `mmc3_irq_tests/5.MMC3_rev_A.nes` | tela | Pass | ✅ |  |
| `mmc3_irq_tests/6.MMC3_rev_B.nes` | tela | Pass | ✅ |  |

## Resumo: 115 ✅ · 1 ❌ esperados · 0 ⚠️ · 0 🔴 · 0 ⏭ (de 116 ROMs)
