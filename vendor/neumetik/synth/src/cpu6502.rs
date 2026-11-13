// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
//! The NES's 6502 (the 2A03's core: no decimal mode), written from the public opcode tables: every official
//! opcode and the stable unofficial ones NSF rips use (LAX, SAX, DCP, ISC, SLO, RLA, SRE, RRA, ANC, ALR, ARR,
//! SBX, the extra NOPs and SBC), with their cycle counts and page-crossing penalties. Enough for music
//! drivers: it runs a routine until it returns ([`Cpu::call`]) and takes the IRQ line between instructions.

/// What the CPU sees at each address.
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, v: u8);
    /// Whether an interrupt is asserted (the DMC's, the frame counter's, a mapper's).
    fn irq(&mut self) -> bool {
        false
    }
    /// The cycles just run, for the bus's own clocks.
    fn tick(&mut self, _cycles: u32) {}
}

const C: u8 = 0x01;
const Z: u8 = 0x02;
const I: u8 = 0x04;
const D: u8 = 0x08;
const B: u8 = 0x10;
const U: u8 = 0x20;
const V: u8 = 0x40;
const N: u8 = 0x80;

/// Where [`Cpu::call`] returns to: an address nothing executes (the CPU waits there between calls).
pub const RETURN: u16 = 0x5ff5;

#[derive(Clone, Debug)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub p: u8,
    pub pc: u16,
    pub cycles: u64,
    /// A KIL opcode stopped it.
    pub jammed: bool,
}

impl Default for Cpu {
    fn default() -> Cpu {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            s: 0xfd,
            p: I | U,
            pc: 0,
            cycles: 0,
            jammed: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Imp,
    Imm,
    Zp,
    Zpx,
    Zpy,
    Abs,
    Abx,
    Aby,
    Izx,
    Izy,
    Ind,
    Rel,
}

impl Cpu {
    fn push<M: Bus>(&mut self, m: &mut M, v: u8) {
        m.write(0x100 | u16::from(self.s), v);
        self.s = self.s.wrapping_sub(1);
    }

    fn pull<M: Bus>(&mut self, m: &mut M) -> u8 {
        self.s = self.s.wrapping_add(1);
        m.read(0x100 | u16::from(self.s))
    }

    fn fetch<M: Bus>(&mut self, m: &mut M) -> u8 {
        let v = m.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    fn fetch16<M: Bus>(&mut self, m: &mut M) -> u16 {
        let lo = self.fetch(m);
        let hi = self.fetch(m);
        u16::from_le_bytes([lo, hi])
    }

    fn nz(&mut self, v: u8) {
        self.p = (self.p & !(N | Z)) | (v & N) | if v == 0 { Z } else { 0 };
    }

    fn flag(&mut self, f: u8, on: bool) {
        if on {
            self.p |= f;
        } else {
            self.p &= !f;
        }
    }

    /// The operand's address, and whether indexing crossed a page.
    fn address<M: Bus>(&mut self, m: &mut M, mode: Mode) -> (u16, bool) {
        match mode {
            Mode::Imp => (0, false),
            Mode::Imm => {
                let a = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (a, false)
            }
            Mode::Zp => (u16::from(self.fetch(m)), false),
            Mode::Zpx => (u16::from(self.fetch(m).wrapping_add(self.x)), false),
            Mode::Zpy => (u16::from(self.fetch(m).wrapping_add(self.y)), false),
            Mode::Abs => (self.fetch16(m), false),
            Mode::Abx => {
                let b = self.fetch16(m);
                let a = b.wrapping_add(u16::from(self.x));
                (a, a & 0xff00 != b & 0xff00)
            }
            Mode::Aby => {
                let b = self.fetch16(m);
                let a = b.wrapping_add(u16::from(self.y));
                (a, a & 0xff00 != b & 0xff00)
            }
            Mode::Izx => {
                let z = self.fetch(m).wrapping_add(self.x);
                let lo = m.read(u16::from(z));
                let hi = m.read(u16::from(z.wrapping_add(1)));
                (u16::from_le_bytes([lo, hi]), false)
            }
            Mode::Izy => {
                let z = self.fetch(m);
                let lo = m.read(u16::from(z));
                let hi = m.read(u16::from(z.wrapping_add(1)));
                let b = u16::from_le_bytes([lo, hi]);
                let a = b.wrapping_add(u16::from(self.y));
                (a, a & 0xff00 != b & 0xff00)
            }
            Mode::Ind => {
                // The page-wrap bug: ($xxFF) takes its high byte from $xx00.
                let p = self.fetch16(m);
                let lo = m.read(p);
                let hi = m.read((p & 0xff00) | (p.wrapping_add(1) & 0x00ff));
                (u16::from_le_bytes([lo, hi]), false)
            }
            Mode::Rel => {
                let o = self.fetch(m) as i8;
                (self.pc.wrapping_add(o as u16), false)
            }
        }
    }

    fn adc(&mut self, v: u8) {
        let s = u16::from(self.a) + u16::from(v) + u16::from(self.p & C);
        let r = s as u8;
        self.flag(V, (!(self.a ^ v) & (self.a ^ r) & 0x80) != 0);
        self.flag(C, s > 0xff);
        self.a = r;
        self.nz(r);
    }

    fn cmp(&mut self, reg: u8, v: u8) {
        self.flag(C, reg >= v);
        self.nz(reg.wrapping_sub(v));
    }

    fn asl(&mut self, v: u8) -> u8 {
        self.flag(C, v & 0x80 != 0);
        let r = v << 1;
        self.nz(r);
        r
    }

    fn lsr(&mut self, v: u8) -> u8 {
        self.flag(C, v & 1 != 0);
        let r = v >> 1;
        self.nz(r);
        r
    }

    fn rol(&mut self, v: u8) -> u8 {
        let c = self.p & C;
        self.flag(C, v & 0x80 != 0);
        let r = (v << 1) | c;
        self.nz(r);
        r
    }

    fn ror(&mut self, v: u8) -> u8 {
        let c = self.p & C;
        self.flag(C, v & 1 != 0);
        let r = (v >> 1) | (c << 7);
        self.nz(r);
        r
    }

    fn branch(&mut self, take: bool, target: u16) -> u32 {
        if !take {
            return 0;
        }
        let extra = if target & 0xff00 != self.pc & 0xff00 { 2 } else { 1 };
        self.pc = target;
        extra
    }

    /// Takes the IRQ (when asserted and not masked), else runs one instruction. Returns its cycles.
    pub fn step<M: Bus>(&mut self, m: &mut M) -> u32 {
        if self.jammed {
            m.tick(1);
            self.cycles += 1;
            return 1;
        }
        if self.p & I == 0 && m.irq() {
            let pc = self.pc;
            self.push(m, (pc >> 8) as u8);
            self.push(m, pc as u8);
            let p = (self.p | U) & !B;
            self.push(m, p);
            self.p |= I;
            self.pc = u16::from_le_bytes([m.read(0xfffe), m.read(0xffff)]);
            m.tick(7);
            self.cycles += 7;
            return 7;
        }
        let op = self.fetch(m);
        let c = self.exec(m, op);
        m.tick(c);
        self.cycles += u64::from(c);
        c
    }

    fn exec<M: Bus>(&mut self, m: &mut M, op: u8) -> u32 {
        use Mode::*;
        // Reads: the operand's value, plus a cycle on a page crossing where the opcode has one.
        macro_rules! rd {
            ($mode:expr) => {{
                let (a, x) = self.address(m, $mode);
                (m.read(a), u32::from(x))
            }};
        }
        // Read-modify-write: the address, read, written back (the 6502 writes the old value first).
        macro_rules! rmw {
            ($mode:expr, $f:expr) => {{
                let (a, _) = self.address(m, $mode);
                let v = m.read(a);
                m.write(a, v);
                let f: fn(&mut Cpu, u8) -> u8 = $f;
                let r = f(self, v);
                m.write(a, r);
                r
            }};
        }
        let (mode, base) = match op {
            // Load and store.
            0xa9 => (Imm, 2),
            0xa5 => (Zp, 3),
            0xb5 => (Zpx, 4),
            0xad => (Abs, 4),
            0xbd => (Abx, 4),
            0xb9 => (Aby, 4),
            0xa1 => (Izx, 6),
            0xb1 => (Izy, 5),
            _ => (Imp, 0),
        };
        if base != 0 {
            let (v, x) = rd!(mode);
            self.a = v;
            self.nz(v);
            return base + x;
        }
        match op {
            0xa2 | 0xa6 | 0xb6 | 0xae | 0xbe => {
                let (mode, base) = match op {
                    0xa2 => (Imm, 2),
                    0xa6 => (Zp, 3),
                    0xb6 => (Zpy, 4),
                    0xae => (Abs, 4),
                    _ => (Aby, 4),
                };
                let (v, x) = rd!(mode);
                self.x = v;
                self.nz(v);
                base + x
            }
            0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => {
                let (mode, base) = match op {
                    0xa0 => (Imm, 2),
                    0xa4 => (Zp, 3),
                    0xb4 => (Zpx, 4),
                    0xac => (Abs, 4),
                    _ => (Abx, 4),
                };
                let (v, x) = rd!(mode);
                self.y = v;
                self.nz(v);
                base + x
            }
            0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                let (mode, c) = match op {
                    0x85 => (Zp, 3),
                    0x95 => (Zpx, 4),
                    0x8d => (Abs, 4),
                    0x9d => (Abx, 5),
                    0x99 => (Aby, 5),
                    0x81 => (Izx, 6),
                    _ => (Izy, 6),
                };
                let (a, _) = self.address(m, mode);
                m.write(a, self.a);
                c
            }
            0x86 | 0x96 | 0x8e => {
                let (mode, c) = match op {
                    0x86 => (Zp, 3),
                    0x96 => (Zpy, 4),
                    _ => (Abs, 4),
                };
                let (a, _) = self.address(m, mode);
                m.write(a, self.x);
                c
            }
            0x84 | 0x94 | 0x8c => {
                let (mode, c) = match op {
                    0x84 => (Zp, 3),
                    0x94 => (Zpx, 4),
                    _ => (Abs, 4),
                };
                let (a, _) = self.address(m, mode);
                m.write(a, self.y);
                c
            }
            // Transfers.
            0xaa => {
                self.x = self.a;
                self.nz(self.x);
                2
            }
            0xa8 => {
                self.y = self.a;
                self.nz(self.y);
                2
            }
            0x8a => {
                self.a = self.x;
                self.nz(self.a);
                2
            }
            0x98 => {
                self.a = self.y;
                self.nz(self.a);
                2
            }
            0xba => {
                self.x = self.s;
                self.nz(self.x);
                2
            }
            0x9a => {
                self.s = self.x;
                2
            }
            // Stack.
            0x48 => {
                self.push(m, self.a);
                3
            }
            0x08 => {
                self.push(m, self.p | B | U);
                3
            }
            0x68 => {
                self.a = self.pull(m);
                self.nz(self.a);
                4
            }
            0x28 => {
                self.p = (self.pull(m) & !B) | U;
                4
            }
            // Arithmetic and logic: ORA, AND, EOR, ADC, CMP, SBC.
            0x01 | 0x05 | 0x09 | 0x0d | 0x11 | 0x15 | 0x19 | 0x1d | 0x21 | 0x25 | 0x29 | 0x2d | 0x31 | 0x35 | 0x39 | 0x3d | 0x41 | 0x45 | 0x49 | 0x4d | 0x51 | 0x55 | 0x59 | 0x5d | 0x61 | 0x65
            | 0x69 | 0x6d | 0x71 | 0x75 | 0x79 | 0x7d | 0xc1 | 0xc5 | 0xc9 | 0xcd | 0xd1 | 0xd5 | 0xd9 | 0xdd | 0xe1 | 0xe5 | 0xe9 | 0xeb | 0xed | 0xf1 | 0xf5 | 0xf9 | 0xfd => {
                let (mode, base) = match op & 0x1f {
                    0x01 => (Izx, 6),
                    0x05 => (Zp, 3),
                    0x09 | 0x0b => (Imm, 2),
                    0x0d => (Abs, 4),
                    0x11 => (Izy, 5),
                    0x15 => (Zpx, 4),
                    0x19 => (Aby, 4),
                    _ => (Abx, 4),
                };
                let (v, x) = rd!(mode);
                match op >> 5 {
                    0 => {
                        self.a |= v;
                        self.nz(self.a);
                    }
                    1 => {
                        self.a &= v;
                        self.nz(self.a);
                    }
                    2 => {
                        self.a ^= v;
                        self.nz(self.a);
                    }
                    3 => self.adc(v),
                    6 => self.cmp(self.a, v),
                    _ => self.adc(!v),
                }
                base + x
            }
            0xe0 | 0xe4 | 0xec | 0xc0 | 0xc4 | 0xcc => {
                let (mode, base) = match op & 0x0f {
                    0x00 => (Imm, 2),
                    0x04 => (Zp, 3),
                    _ => (Abs, 4),
                };
                let (v, _) = rd!(mode);
                let r = if op >= 0xe0 { self.x } else { self.y };
                self.cmp(r, v);
                base
            }
            0x24 | 0x2c => {
                let (v, _) = rd!(if op == 0x24 { Zp } else { Abs });
                self.flag(Z, self.a & v == 0);
                self.p = (self.p & !(N | V)) | (v & (N | V));
                if op == 0x24 {
                    3
                } else {
                    4
                }
            }
            // Shifts and increments: ASL, ROL, LSR, ROR, DEC, INC.
            0x0a => {
                self.a = self.asl(self.a);
                2
            }
            0x2a => {
                self.a = self.rol(self.a);
                2
            }
            0x4a => {
                self.a = self.lsr(self.a);
                2
            }
            0x6a => {
                self.a = self.ror(self.a);
                2
            }
            0x06 | 0x16 | 0x0e | 0x1e | 0x26 | 0x36 | 0x2e | 0x3e | 0x46 | 0x56 | 0x4e | 0x5e | 0x66 | 0x76 | 0x6e | 0x7e | 0xc6 | 0xd6 | 0xce | 0xde | 0xe6 | 0xf6 | 0xee | 0xfe => {
                let (mode, c) = match op & 0x1f {
                    0x06 => (Zp, 5),
                    0x16 => (Zpx, 6),
                    0x0e => (Abs, 6),
                    _ => (Abx, 7),
                };
                let f: fn(&mut Cpu, u8) -> u8 = match op >> 5 {
                    0 => Cpu::asl,
                    1 => Cpu::rol,
                    2 => Cpu::lsr,
                    3 => Cpu::ror,
                    6 => |c, v| {
                        let r = v.wrapping_sub(1);
                        c.nz(r);
                        r
                    },
                    _ => |c, v| {
                        let r = v.wrapping_add(1);
                        c.nz(r);
                        r
                    },
                };
                let (a, _) = self.address(m, mode);
                let v = m.read(a);
                m.write(a, v);
                let r = f(self, v);
                m.write(a, r);
                c
            }
            0xca => {
                self.x = self.x.wrapping_sub(1);
                self.nz(self.x);
                2
            }
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.nz(self.y);
                2
            }
            0xe8 => {
                self.x = self.x.wrapping_add(1);
                self.nz(self.x);
                2
            }
            0xc8 => {
                self.y = self.y.wrapping_add(1);
                self.nz(self.y);
                2
            }
            // Jumps and calls.
            0x4c => {
                self.pc = self.fetch16(m);
                3
            }
            0x6c => {
                self.pc = self.address(m, Ind).0;
                5
            }
            0x20 => {
                let t = self.fetch16(m);
                let r = self.pc.wrapping_sub(1);
                self.push(m, (r >> 8) as u8);
                self.push(m, r as u8);
                self.pc = t;
                6
            }
            0x60 => {
                let lo = self.pull(m);
                let hi = self.pull(m);
                self.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                6
            }
            0x40 => {
                self.p = (self.pull(m) & !B) | U;
                let lo = self.pull(m);
                let hi = self.pull(m);
                self.pc = u16::from_le_bytes([lo, hi]);
                6
            }
            0x00 => {
                let r = self.pc.wrapping_add(1);
                self.push(m, (r >> 8) as u8);
                self.push(m, r as u8);
                self.push(m, self.p | B | U);
                self.p |= I;
                self.pc = u16::from_le_bytes([m.read(0xfffe), m.read(0xffff)]);
                7
            }
            // Branches.
            0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xb0 | 0xd0 | 0xf0 => {
                let (t, _) = self.address(m, Rel);
                let f = [N, V, C, Z][usize::from(op >> 6)];
                let set = self.p & f != 0;
                2 + self.branch(set == (op & 0x20 != 0), t)
            }
            // Flags.
            0x18 => {
                self.p &= !C;
                2
            }
            0x38 => {
                self.p |= C;
                2
            }
            0x58 => {
                self.p &= !I;
                2
            }
            0x78 => {
                self.p |= I;
                2
            }
            0xb8 => {
                self.p &= !V;
                2
            }
            0xd8 => {
                self.p &= !D;
                2
            }
            0xf8 => {
                self.p |= D;
                2
            }
            0xea | 0x1a | 0x3a | 0x5a | 0x7a | 0xda | 0xfa => 2,
            // Unofficial: NOPs that read.
            0x80 | 0x82 | 0x89 | 0xc2 | 0xe2 => {
                self.pc = self.pc.wrapping_add(1);
                2
            }
            0x04 | 0x44 | 0x64 => {
                rd!(Zp);
                3
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 => {
                rd!(Zpx);
                4
            }
            0x0c => {
                rd!(Abs);
                4
            }
            0x1c | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => {
                let (_, x) = rd!(Abx);
                4 + x
            }
            // LAX, SAX.
            0xa7 | 0xb7 | 0xaf | 0xbf | 0xa3 | 0xb3 | 0xab => {
                let (mode, base) = match op {
                    0xa7 => (Zp, 3),
                    0xb7 => (Zpy, 4),
                    0xaf => (Abs, 4),
                    0xbf => (Aby, 4),
                    0xa3 => (Izx, 6),
                    0xb3 => (Izy, 5),
                    _ => (Imm, 2),
                };
                let (v, x) = rd!(mode);
                self.a = v;
                self.x = v;
                self.nz(v);
                base + x
            }
            0x87 | 0x97 | 0x8f | 0x83 => {
                let (mode, c) = match op {
                    0x87 => (Zp, 3),
                    0x97 => (Zpy, 4),
                    0x8f => (Abs, 4),
                    _ => (Izx, 6),
                };
                let (a, _) = self.address(m, mode);
                m.write(a, self.a & self.x);
                c
            }
            // SLO, RLA, SRE, RRA, DCP, ISC.
            0x03 | 0x07 | 0x0f | 0x13 | 0x17 | 0x1b | 0x1f | 0x23 | 0x27 | 0x2f | 0x33 | 0x37 | 0x3b | 0x3f | 0x43 | 0x47 | 0x4f | 0x53 | 0x57 | 0x5b | 0x5f | 0x63 | 0x67 | 0x6f | 0x73 | 0x77
            | 0x7b | 0x7f | 0xc3 | 0xc7 | 0xcf | 0xd3 | 0xd7 | 0xdb | 0xdf | 0xe3 | 0xe7 | 0xef | 0xf3 | 0xf7 | 0xfb | 0xff => {
                let (mode, c) = match op & 0x1f {
                    0x03 => (Izx, 8),
                    0x07 => (Zp, 5),
                    0x0f => (Abs, 6),
                    0x13 => (Izy, 8),
                    0x17 => (Zpx, 6),
                    0x1b => (Aby, 7),
                    _ => (Abx, 7),
                };
                match op >> 5 {
                    0 => {
                        let r = rmw!(mode, Cpu::asl);
                        self.a |= r;
                        self.nz(self.a);
                    }
                    1 => {
                        let r = rmw!(mode, Cpu::rol);
                        self.a &= r;
                        self.nz(self.a);
                    }
                    2 => {
                        let r = rmw!(mode, Cpu::lsr);
                        self.a ^= r;
                        self.nz(self.a);
                    }
                    3 => {
                        let r = rmw!(mode, Cpu::ror);
                        self.adc(r);
                    }
                    6 => {
                        let r = rmw!(mode, |_, v| v.wrapping_sub(1));
                        self.cmp(self.a, r);
                    }
                    _ => {
                        let r = rmw!(mode, |_, v| v.wrapping_add(1));
                        self.adc(!r);
                    }
                }
                c
            }
            0x0b | 0x2b => {
                let (v, _) = rd!(Imm);
                self.a &= v;
                self.nz(self.a);
                self.flag(C, self.a & 0x80 != 0);
                2
            }
            0x4b => {
                let (v, _) = rd!(Imm);
                self.a &= v;
                self.a = self.lsr(self.a);
                2
            }
            0x6b => {
                let (v, _) = rd!(Imm);
                self.a &= v;
                self.a = (self.a >> 1) | ((self.p & C) << 7);
                self.nz(self.a);
                self.flag(C, self.a & 0x40 != 0);
                self.flag(V, ((self.a >> 6) ^ (self.a >> 5)) & 1 != 0);
                2
            }
            0xcb => {
                let (v, _) = rd!(Imm);
                let ax = self.a & self.x;
                self.flag(C, ax >= v);
                self.x = ax.wrapping_sub(v);
                self.nz(self.x);
                2
            }
            // KIL: the CPU stops.
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xb2 | 0xd2 | 0xf2 => {
                self.jammed = true;
                self.pc = self.pc.wrapping_sub(1);
                2
            }
            // The unstable rest (SHA, SHX, SHY, TAS, LAS, XAA): their usual results.
            0x9e => {
                let (a, _) = self.address(m, Aby);
                m.write(a, self.x & ((a >> 8) as u8).wrapping_add(1));
                5
            }
            0x9c => {
                let (a, _) = self.address(m, Abx);
                m.write(a, self.y & ((a >> 8) as u8).wrapping_add(1));
                5
            }
            0x9f | 0x93 => {
                let (a, _) = self.address(m, if op == 0x9f { Aby } else { Izy });
                m.write(a, self.a & self.x & ((a >> 8) as u8).wrapping_add(1));
                if op == 0x9f {
                    5
                } else {
                    6
                }
            }
            0x9b => {
                let (a, _) = self.address(m, Aby);
                self.s = self.a & self.x;
                m.write(a, self.s & ((a >> 8) as u8).wrapping_add(1));
                5
            }
            0xbb => {
                let (v, x) = rd!(Aby);
                let r = v & self.s;
                self.a = r;
                self.x = r;
                self.s = r;
                self.nz(r);
                4 + x
            }
            0x8b => {
                let (v, _) = rd!(Imm);
                self.a = self.x & v;
                self.nz(self.a);
                2
            }
            _ => 2,
        }
    }

    /// Calls the routine at `addr` (JSR) and runs it until it returns, or `limit` cycles have gone by.
    /// Returns whether it returned.
    pub fn call<M: Bus>(&mut self, m: &mut M, addr: u16, limit: u64) -> bool {
        let r = RETURN.wrapping_sub(1);
        self.push(m, (r >> 8) as u8);
        self.push(m, r as u8);
        self.pc = addr;
        let end = self.cycles + limit;
        while self.cycles < end {
            if self.pc == RETURN {
                return true;
            }
            if self.jammed {
                return false;
            }
            self.step(m);
        }
        false
    }

    /// Runs until `cycles` have gone by (for a routine that never returns, or idling between calls with the IRQ
    /// line live).
    pub fn run<M: Bus>(&mut self, m: &mut M, cycles: u64) {
        let end = self.cycles + cycles;
        while self.cycles < end {
            self.step(m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram(Vec<u8>);

    impl Bus for Ram {
        fn read(&mut self, a: u16) -> u8 {
            self.0[usize::from(a)]
        }
        fn write(&mut self, a: u16, v: u8) {
            self.0[usize::from(a)] = v;
        }
    }

    fn run(code: &[u8]) -> (Cpu, Ram) {
        let mut m = Ram(vec![0; 0x10000]);
        m.0[0x8000..0x8000 + code.len()].copy_from_slice(code);
        let mut c = Cpu::default();
        assert!(c.call(&mut m, 0x8000, 100_000));
        (c, m)
    }

    #[test]
    fn adds_with_carry_and_overflow() {
        // LDA #$50, CLC, ADC #$50: $A0, V set, C clear; SEC, SBC #$F0: $B0 (borrow), RTS.
        let (c, _) = run(&[0xa9, 0x50, 0x18, 0x69, 0x50, 0x60]);
        assert_eq!(c.a, 0xa0);
        assert_eq!(c.p & (V | C | N), V | N);
        let (c, _) = run(&[0xa9, 0xa0, 0x38, 0xe9, 0xf0, 0x60]);
        assert_eq!(c.a, 0xb0);
        assert_eq!(c.p & C, 0);
    }

    #[test]
    fn loops_and_indirect_stores() {
        // Fill $0200..$020F with 0..15 through ($10),Y, counting down with DEY/BPL; JSR a subroutine that INXs.
        let code = [
            0xa9, 0x00, 0x85, 0x10, 0xa9, 0x02, 0x85, 0x11, // ($10) = $0200
            0xa0, 0x0f, // LDY #15
            0x98, 0x91, 0x10, 0x88, 0x10, 0xfa, // loop: TYA, STA ($10),Y, DEY, BPL loop
            0x20, 0x20, 0x80, 0x60, // JSR $8020, RTS
        ];
        let mut all = code.to_vec();
        all.resize(0x20, 0xea);
        all.extend_from_slice(&[0xe8, 0xe8, 0x60]);
        let (c, m) = run(&all);
        assert_eq!(&m.0[0x200..0x210], &(0..16).collect::<Vec<u8>>()[..]);
        assert_eq!(c.x, 2);
        assert_eq!(c.s, 0xfd);
    }

    // kevtris's nestest.nes run from $C000 (its automatic mode) against its log: PC, registers and cycle count
    // before every instruction, official and unofficial opcodes alike.
    // NESTEST=dir-with-nestest.nes-and-nestest.log cargo test --release --lib cpu6502 -- --ignored
    #[test]
    #[ignore]
    fn runs_nestest_like_its_log() {
        struct Nes(Vec<u8>, Vec<u8>);
        impl Bus for Nes {
            fn read(&mut self, a: u16) -> u8 {
                match a {
                    0x0000..=0x1fff => self.0[usize::from(a & 0x7ff)],
                    0x8000..=0xffff => self.1[usize::from(a & 0x3fff)],
                    _ => 0xff,
                }
            }
            fn write(&mut self, a: u16, v: u8) {
                if a < 0x2000 {
                    self.0[usize::from(a & 0x7ff)] = v;
                }
            }
        }
        let dir = std::path::PathBuf::from(std::env::var("NESTEST").expect("NESTEST"));
        let rom = std::fs::read(dir.join("nestest.nes")).unwrap();
        let log = std::fs::read_to_string(dir.join("nestest.log")).unwrap();
        let mut m = Nes(vec![0; 0x800], rom[16..16 + 0x4000].to_vec());
        let mut c = Cpu {
            pc: 0xc000,
            cycles: 7,
            ..Cpu::default()
        };
        let hex = |l: &str, k: &str| u8::from_str_radix(&l[l.find(k).unwrap() + k.len()..][..2], 16).unwrap();
        for (n, l) in log.lines().enumerate() {
            let pc = u16::from_str_radix(&l[..4], 16).unwrap();
            let cyc: u64 = l[l.find("CYC:").unwrap() + 4..].trim().parse().unwrap();
            let want = (pc, hex(l, "A:"), hex(l, "X:"), hex(l, "Y:"), hex(l, "P:"), hex(l, "SP:"), cyc);
            let got = (c.pc, c.a, c.x, c.y, c.p, c.s, c.cycles);
            assert_eq!(got, want, "line {}: {l}", n + 1);
            c.step(&mut m);
        }
        // The test's own result codes: 0 at $02 and $03.
        assert_eq!((m.0[2], m.0[3]), (0, 0));
    }

    #[test]
    fn jmp_indirect_wraps_in_its_page() {
        let mut m = Ram(vec![0; 0x10000]);
        m.0[0x8000..0x8003].copy_from_slice(&[0x6c, 0xff, 0x02]);
        m.0[0x02ff] = 0x34;
        m.0[0x0200] = 0x12;
        m.0[0x0300] = 0x99;
        let mut c = Cpu { pc: 0x8000, ..Cpu::default() };
        c.step(&mut m);
        assert_eq!(c.pc, 0x1234);
    }
}
