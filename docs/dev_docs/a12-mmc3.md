# A12 e o contador do MMC3 (F3-02)

O MMC3 não sabe o que é "scanline": ele conta **bordas de subida de A12** no barramento da
PPU. Com a configuração usual (background em `$0000`, sprites em `$1000`) a primeira subida de
cada linha acontece na busca de padrão do sprite 0, por volta do dot 260 — daí a aproximação
"IRQ por scanline" que a maioria dos emuladores simples usa (e que o RNFE usava até F3-01).

## O que o RNFE faz

- `Ppu::bus_addr(addr)` é chamado com **todo** endereço que vai ao barramento: leituras e
  escritas de VRAM (`vram_read`/`vram_write`), as duas leituras de nametable descartadas de
  cada slot de sprite (dots 257–320) e `v` sempre que muda fora do render (`$2006`, incremento
  de `$2007`) — porque com o render desligado o barramento mostra `v`.
- Filtro: a subida só conta se A12 ficou baixo por `A12_FILTER_DOTS = 10` dots (~3 ciclos de
  M2). Isso ignora o vai-e-vem entre os padrões de dois sprites consecutivos (4 dots baixo).
- A borda vira `ppu.a12_rise = true`; o `Bus::tick_post` entrega ao cartucho
  (`Cartridge::a12_rise` → `Mapper::a12_rise`) no mesmo ciclo de CPU.
- A busca de sprites saiu do dot 340 e foi para os dots 257–320, 8 dots por slot, como no
  hardware. Slots vazios buscam o tile `$FF` (é isso que clocka o MMC3 em linhas sem sprite,
  inclusive na pré-render — 241 clocks por frame, teste `2-details`).
- Escrever a paleta por `$2006 = $3F00` também sobe A12 (bit 12 de `$3F00` é 1) — é real.

## MMC3

- `$C000` = latch; `$C001` zera o contador **agora** e marca `reload` (recarrega no próximo
  clock, sem decrementar); `$E000` desabilita e **reconhece** o IRQ (a linha é nível);
  `$E001` habilita; `$A001` bit 7 habilita a PRG RAM e bit 6 protege a escrita.
- Revisão B (Sharp, padrão): IRQ sempre que o contador está em 0 após o clock — com latch 0,
  IRQ a cada clock.
- Revisão A (NEC, submapper NES 2.0 **4.4**): IRQ só quando o contador *passou* a 0 neste
  clock, ou recarregou depois de `$C001`.
- As ROMs `6-MMC3_alt` e `5.MMC3_rev_A` são iNES: o harness (`ts(..., submapper)`) reescreve o
  header como NES 2.0 4.4 antes de carregar.

Resultado: mmc3_test_2 6/6 e mmc3_irq_tests 6/6, snapshots inalterados.
