#!/usr/bin/env bash
# Baixa (clone esparso) as ROMs de teste de christopherpow/nes-test-roms em test-roms/.
# Sobrescreva o destino com RNFE_TEST_ROMS=/outro/dir.
set -euo pipefail
DIR="${RNFE_TEST_ROMS:-$(cd "$(dirname "$0")/.." && pwd)/test-roms}"
REPO=https://github.com/christopherpow/nes-test-roms
# Commit fixo: os hashes de snapshot e os CRCs esperados dependem destes arquivos exatos.
SHA=95d8f621ae55cee0d09b91519a8989ae0e64753b
SUBDIRS=(other instr_test-v5 instr_timing instr_misc cpu_interrupts_v2 cpu_dummy_reads cpu_dummy_writes
  cpu_exec_space cpu_reset branch_timing_tests ppu_vbl_nmi vbl_nmi_timing sprite_hit_tests_2005.10.05
  sprite_overflow_tests oam_read oam_stress ppu_open_bus ppu_read_buffer full_palette nmi_sync scanline
  apu_test blargg_apu_2005.07.30 apu_reset apu_mixer dmc_tests dmc_dma_during_read4
  mmc3_test_2 mmc3_irq_tests MMC1_A12 read_joy3 mmc5test mmc5test_v2 exram)
if [ -d "$DIR/.git" ]; then
  git -C "$DIR" sparse-checkout set "${SUBDIRS[@]}"
  if [ "$(git -C "$DIR" rev-parse HEAD)" != "$SHA" ]; then
    git -C "$DIR" fetch -q --depth 1 origin "$SHA" && git -C "$DIR" checkout -q FETCH_HEAD || true
  fi
else
  git clone -q --depth 1 --filter=blob:none --sparse "$REPO" "$DIR"
  git -C "$DIR" sparse-checkout set "${SUBDIRS[@]}"
  git -C "$DIR" fetch -q --depth 1 origin "$SHA" && git -C "$DIR" checkout -q FETCH_HEAD
fi
for f in other/nestest.nes other/nestest.log instr_test-v5/rom_singles/01-basics.nes; do
  [ -f "$DIR/$f" ] || { echo "faltou $f" >&2; exit 1; }
done
head -c 4 "$DIR/other/nestest.nes" | grep -q $'NES\x1a' || { echo "nestest.nes corrompido" >&2; exit 1; }
echo "ROMs em $DIR ($(find "$DIR" -iname '*.nes' | wc -l) arquivos)"
