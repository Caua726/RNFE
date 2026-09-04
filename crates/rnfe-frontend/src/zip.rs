//! Leitura de `.zip` só o suficiente para tirar a ROM de dentro: os packs de NES vêm zipados e
//! descompactar antes é atrito puro no celular.
//!
//! Sem dependências: um decodificador DEFLATE (blocos armazenados, Huffman fixo e dinâmico) e o
//! percurso dos cabeçalhos locais. Não faz CRC nem zip64 — se algo não bate, devolve `None` e o
//! chamador segue com a mensagem de erro normal.

/// Assinatura de um arquivo zip (cabeçalho local do primeiro membro).
pub const MAGIC: &[u8; 4] = b"PK\x03\x04";

/// Nome e conteúdo do primeiro `.nes` de dentro do zip.
pub fn extract_nes(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    for (name, data) in entries(bytes) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".nes") {
            let out = data?;
            return Some((name, out));
        }
    }
    None
}

/// Itera os membros pelo cabeçalho local. O conteúdo é `None` quando o método de compressão não
/// é suportado ou os dados estão truncados.
fn entries(bytes: &[u8]) -> impl Iterator<Item = (String, Option<Vec<u8>>)> + '_ {
    let mut pos = 0usize;
    std::iter::from_fn(move || {
        loop {
            if pos + 30 > bytes.len() || &bytes[pos..pos + 4] != MAGIC {
                return None;
            }
            let flags = u16le(bytes, pos + 6);
            let method = u16le(bytes, pos + 8);
            let mut comp = u32le(bytes, pos + 18) as usize;
            let mut raw = u32le(bytes, pos + 22) as usize;
            let name_len = u16le(bytes, pos + 26) as usize;
            let extra_len = u16le(bytes, pos + 28) as usize;
            let name_at = pos + 30;
            let data_at = name_at + name_len + extra_len;
            if data_at > bytes.len() {
                return None;
            }
            let name = String::from_utf8_lossy(&bytes[name_at..name_at + name_len]).into_owned();
            // Bit 3: tamanhos só existem no descritor depois dos dados — sem índice central não
            // dá para saber onde o membro acaba, então paramos aqui.
            if flags & 0x08 != 0 && comp == 0 {
                return Some((name, None));
            }
            if data_at + comp > bytes.len() {
                comp = bytes.len() - data_at;
                raw = raw.min(comp * 1024);
            }
            let body = &bytes[data_at..data_at + comp];
            let out = match method {
                0 => Some(body.to_vec()),
                8 => inflate(body, raw),
                _ => None,
            };
            pos = data_at + comp;
            if name.ends_with('/') {
                continue; // diretório
            }
            return Some((name, out));
        }
    })
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

// ------------------------------------------------------------------ DEFLATE

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    acc: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, bit: 0, acc: 0 }
    }

    fn need(&mut self, n: u32) -> Option<()> {
        while self.bit < n {
            let byte = *self.data.get(self.pos)? as u32;
            self.pos += 1;
            self.acc |= byte << self.bit;
            self.bit += 8;
        }
        Some(())
    }

    fn take(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        self.need(n)?;
        let v = self.acc & ((1u32 << n) - 1);
        self.acc >>= n;
        self.bit -= n;
        Some(v)
    }

    /// Alinha no próximo byte (blocos armazenados).
    fn align(&mut self) {
        let drop = self.bit % 8;
        self.acc >>= drop;
        self.bit -= drop;
    }
}

/// Árvore canônica de Huffman: contagens por comprimento + símbolos em ordem.
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Huffman {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Huffman { counts, symbols }
    }

    fn decode(&self, bits: &mut Bits) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.take(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195,
    227, 258,
];
const LEN_EXTRA: [u8; 29] =
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073,
    4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// DEFLATE cru (sem cabeçalho zlib). `hint` é o tamanho esperado, só para reservar memória.
pub fn inflate(data: &[u8], hint: usize) -> Option<Vec<u8>> {
    let mut bits = Bits::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(hint.min(64 << 20));
    loop {
        let last = bits.take(1)?;
        match bits.take(2)? {
            0 => {
                bits.align();
                let len = bits.take(16)? as usize;
                let nlen = bits.take(16)? as usize;
                if len != !nlen & 0xFFFF {
                    return None;
                }
                for _ in 0..len {
                    out.push(bits.take(8)? as u8);
                }
            }
            1 => {
                // Huffman fixo (RFC 1951 §3.2.6)
                let mut lit = [0u8; 288];
                for (i, l) in lit.iter_mut().enumerate() {
                    *l = match i {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                let dist = [5u8; 30];
                block(&mut bits, &mut out, &Huffman::new(&lit), &Huffman::new(&dist))?;
            }
            2 => {
                let hlit = bits.take(5)? as usize + 257;
                let hdist = bits.take(5)? as usize + 1;
                let hclen = bits.take(4)? as usize + 4;
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut code_len = [0u8; 19];
                for &o in ORDER.iter().take(hclen) {
                    code_len[o] = bits.take(3)? as u8;
                }
                let code_tree = Huffman::new(&code_len);
                let mut lengths = vec![0u8; hlit + hdist];
                let mut i = 0;
                while i < lengths.len() {
                    let sym = code_tree.decode(&mut bits)?;
                    match sym {
                        0..=15 => {
                            lengths[i] = sym as u8;
                            i += 1;
                        }
                        16 => {
                            let prev = *lengths.get(i.checked_sub(1)?)?;
                            let n = 3 + bits.take(2)? as usize;
                            for _ in 0..n {
                                *lengths.get_mut(i)? = prev;
                                i += 1;
                            }
                        }
                        17 => i += 3 + bits.take(3)? as usize,
                        18 => i += 11 + bits.take(7)? as usize,
                        _ => return None,
                    }
                }
                if i > lengths.len() {
                    return None;
                }
                let lit = Huffman::new(&lengths[..hlit]);
                let dist = Huffman::new(&lengths[hlit..]);
                block(&mut bits, &mut out, &lit, &dist)?;
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}

fn block(bits: &mut Bits, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) -> Option<()> {
    loop {
        let sym = lit.decode(bits)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Some(()),
            257..=285 => {
                let i = sym as usize - 257;
                let len = LEN_BASE[i] as usize + bits.take(LEN_EXTRA[i] as u32)? as usize;
                let d = dist.decode(bits)? as usize;
                if d >= DIST_BASE.len() {
                    return None;
                }
                let distance = DIST_BASE[d] as usize + bits.take(DIST_EXTRA[d] as u32)? as usize;
                if distance > out.len() {
                    return None;
                }
                let start = out.len() - distance;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_block_roundtrip() {
        // bloco armazenado: bfinal=1, btype=00, len/nlen, dados
        let payload = b"NES\x1a hello";
        let mut d = vec![0x01, payload.len() as u8, 0x00];
        d.push(!(payload.len() as u8));
        d.push(0xFF);
        d.extend_from_slice(payload);
        assert_eq!(inflate(&d, 0).as_deref(), Some(&payload[..]));
    }

    #[test]
    fn no_nes_inside() {
        assert!(extract_nes(b"nao e um zip").is_none());
    }
}
