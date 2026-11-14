//! A Motorola 68000 interpreter: the integer instruction set, in supervisor mode only (the sound driver never leaves it), without cycle counts, traps
//! or interrupts. Written from the instruction set's encodings; its job is to run the original MDSDRV for `mdsdrv`'s comparison.
//! An odd address on a word or long access is an error, as it is a crash on the real chip.

pub type R<T> = Result<T, String>;

pub trait Bus {
    fn read8(&mut self, a: u32) -> u8;
    fn write8(&mut self, a: u32, v: u8);
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sz {
    B,
    W,
    L,
}

impl Sz {
    fn mask(self) -> u32 {
        match self {
            Sz::B => 0xff,
            Sz::W => 0xffff,
            Sz::L => 0xffff_ffff,
        }
    }
    fn msb(self) -> u32 {
        match self {
            Sz::B => 0x80,
            Sz::W => 0x8000,
            Sz::L => 0x8000_0000,
        }
    }
    fn bytes(self) -> u32 {
        match self {
            Sz::B => 1,
            Sz::W => 2,
            Sz::L => 4,
        }
    }
    fn from_bits(b: u16) -> Option<Sz> {
        match b {
            0 => Some(Sz::B),
            1 => Some(Sz::W),
            2 => Some(Sz::L),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Loc {
    D(usize),
    A(usize),
    M(u32),
    Imm(u32),
}

const X: u16 = 0x10;
const N: u16 = 8;
const Z: u16 = 4;
const V: u16 = 2;
const C: u16 = 1;

pub struct Cpu {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
    pub steps: u64,
    /// The last program counters, for a message when something goes wrong.
    pub recent: [u32; 12],
}

pub const HALT: u32 = 0x00ff_fff0;

impl Default for Cpu {
    fn default() -> Cpu {
        Cpu { d: [0; 8], a: [0; 8], pc: 0, sr: 0x2700, steps: 0, recent: [0; 12] }
    }
}

impl Cpu {
    fn r16(&self, bus: &mut dyn Bus, a: u32) -> R<u16> {
        if a & 1 != 0 {
            return Err(format!("address error: word read at {a:#x} (pc {:#x})", self.pc));
        }
        Ok(u16::from(bus.read8(a)) << 8 | u16::from(bus.read8(a.wrapping_add(1))))
    }

    fn w16(&self, bus: &mut dyn Bus, a: u32, v: u16) -> R<()> {
        if a & 1 != 0 {
            return Err(format!("address error: word write at {a:#x} (pc {:#x})", self.pc));
        }
        bus.write8(a, (v >> 8) as u8);
        bus.write8(a.wrapping_add(1), v as u8);
        Ok(())
    }

    fn mread(&self, bus: &mut dyn Bus, a: u32, sz: Sz) -> R<u32> {
        Ok(match sz {
            Sz::B => u32::from(bus.read8(a)),
            Sz::W => u32::from(self.r16(bus, a)?),
            Sz::L => u32::from(self.r16(bus, a)?) << 16 | u32::from(self.r16(bus, a.wrapping_add(2))?),
        })
    }

    fn mwrite(&self, bus: &mut dyn Bus, a: u32, sz: Sz, v: u32) -> R<()> {
        match sz {
            Sz::B => bus.write8(a, v as u8),
            Sz::W => self.w16(bus, a, v as u16)?,
            Sz::L => {
                self.w16(bus, a, (v >> 16) as u16)?;
                self.w16(bus, a.wrapping_add(2), v as u16)?;
            }
        }
        Ok(())
    }

    fn fetch(&mut self, bus: &mut dyn Bus) -> R<u16> {
        let v = self.r16(bus, self.pc)?;
        self.pc = self.pc.wrapping_add(2);
        Ok(v)
    }

    fn fetch32(&mut self, bus: &mut dyn Bus) -> R<u32> {
        Ok(u32::from(self.fetch(bus)?) << 16 | u32::from(self.fetch(bus)?))
    }

    fn index(&mut self, bus: &mut dyn Bus, base: u32) -> R<u32> {
        let e = self.fetch(bus)?;
        let r = usize::from(e >> 12 & 7);
        let v = if e & 0x8000 != 0 { self.a[r] } else { self.d[r] };
        let v = if e & 0x800 != 0 { v } else { v as u16 as i16 as i32 as u32 };
        Ok(base.wrapping_add(v).wrapping_add(e as u8 as i8 as i32 as u32))
    }

    fn loc(&mut self, bus: &mut dyn Bus, mode: u16, reg: u16, sz: Sz) -> R<Loc> {
        let r = usize::from(reg);
        Ok(match mode {
            0 => Loc::D(r),
            1 => Loc::A(r),
            2 => Loc::M(self.a[r]),
            3 => {
                let a = self.a[r];
                let inc = if r == 7 && sz == Sz::B { 2 } else { sz.bytes() };
                self.a[r] = a.wrapping_add(inc);
                Loc::M(a)
            }
            4 => {
                let dec = if r == 7 && sz == Sz::B { 2 } else { sz.bytes() };
                self.a[r] = self.a[r].wrapping_sub(dec);
                Loc::M(self.a[r])
            }
            5 => {
                let d = self.fetch(bus)? as i16 as i32 as u32;
                Loc::M(self.a[r].wrapping_add(d))
            }
            6 => {
                let b = self.a[r];
                Loc::M(self.index(bus, b)?)
            }
            7 => match reg {
                0 => Loc::M(self.fetch(bus)? as i16 as i32 as u32),
                1 => Loc::M(self.fetch32(bus)?),
                2 => {
                    let base = self.pc;
                    let d = self.fetch(bus)? as i16 as i32 as u32;
                    Loc::M(base.wrapping_add(d))
                }
                3 => {
                    let base = self.pc;
                    Loc::M(self.index(bus, base)?)
                }
                4 => Loc::Imm(match sz {
                    Sz::B => u32::from(self.fetch(bus)? & 0xff),
                    Sz::W => u32::from(self.fetch(bus)?),
                    Sz::L => self.fetch32(bus)?,
                }),
                _ => return Err(format!("bad addressing mode 7/{reg} at {:#x}", self.pc)),
            },
            _ => return Err("bad addressing mode".into()),
        })
    }

    fn rd(&self, bus: &mut dyn Bus, l: Loc, sz: Sz) -> R<u32> {
        Ok(match l {
            Loc::D(r) => self.d[r] & sz.mask(),
            Loc::A(r) => self.a[r] & sz.mask(),
            Loc::M(a) => self.mread(bus, a, sz)?,
            Loc::Imm(v) => v,
        })
    }

    fn wr(&mut self, bus: &mut dyn Bus, l: Loc, sz: Sz, v: u32) -> R<()> {
        match l {
            Loc::D(r) => self.d[r] = self.d[r] & !sz.mask() | v & sz.mask(),
            Loc::A(r) => self.a[r] = v,
            Loc::M(a) => self.mwrite(bus, a, sz, v)?,
            Loc::Imm(_) => return Err("write to an immediate".into()),
        }
        Ok(())
    }

    fn flag(&mut self, f: u16, on: bool) {
        if on {
            self.sr |= f;
        } else {
            self.sr &= !f;
        }
    }

    fn nz(&mut self, v: u32, sz: Sz) {
        self.flag(N, v & sz.msb() != 0);
        self.flag(Z, v & sz.mask() == 0);
    }

    fn logic_flags(&mut self, v: u32, sz: Sz) {
        self.nz(v, sz);
        self.flag(V, false);
        self.flag(C, false);
    }

    fn add(&mut self, dst: u32, src: u32, sz: Sz, x: u32, keep_z: bool, set_x: bool) -> u32 {
        let m = sz.mask() as u64;
        let r = (dst as u64 & m) + (src as u64 & m) + u64::from(x);
        let res = (r & m) as u32;
        let c = r > m;
        let v = (!(dst ^ src) & (dst ^ res)) & sz.msb() != 0;
        self.flag(N, res & sz.msb() != 0);
        if !keep_z || res != 0 {
            self.flag(Z, res == 0);
        }
        self.flag(V, v);
        self.flag(C, c);
        if set_x {
            self.flag(X, c);
        }
        res
    }

    /// dst - src - x
    fn sub(&mut self, dst: u32, src: u32, sz: Sz, x: u32, keep_z: bool, set_x: bool) -> u32 {
        let m = sz.mask() as u64;
        let r = (dst as u64 & m).wrapping_sub(src as u64 & m).wrapping_sub(u64::from(x));
        let res = (r & m) as u32;
        let c = (src as u64 & m) + u64::from(x) > (dst as u64 & m);
        let v = ((dst ^ src) & (dst ^ res)) & sz.msb() != 0;
        self.flag(N, res & sz.msb() != 0);
        if !keep_z || res != 0 {
            self.flag(Z, res == 0);
        }
        self.flag(V, v);
        self.flag(C, c);
        if set_x {
            self.flag(X, c);
        }
        res
    }

    fn cond(&self, c: u16) -> bool {
        let (n, z, v, cy) = (self.sr & N != 0, self.sr & Z != 0, self.sr & V != 0, self.sr & C != 0);
        match c {
            0 => true,
            1 => false,
            2 => !cy && !z,
            3 => cy || z,
            4 => !cy,
            5 => cy,
            6 => !z,
            7 => z,
            8 => !v,
            9 => v,
            10 => !n,
            11 => n,
            12 => n == v,
            13 => n != v,
            14 => !z && n == v,
            _ => z || n != v,
        }
    }

    fn push32(&mut self, bus: &mut dyn Bus, v: u32) -> R<()> {
        self.a[7] = self.a[7].wrapping_sub(4);
        self.mwrite(bus, self.a[7], Sz::L, v)
    }

    /// Call the subroutine at `addr` and run until it returns, in at most `max` instructions.
    pub fn call(&mut self, bus: &mut dyn Bus, addr: u32, max: u64) -> R<()> {
        self.push32(bus, HALT)?;
        self.pc = addr;
        let start = self.steps;
        while self.pc != HALT {
            if self.steps - start > max {
                return Err(format!("no return after {max} instructions (pc {:#x}, recently {:x?})", self.pc, self.recent));
            }
            self.step(bus)?;
        }
        Ok(())
    }

    pub fn step(&mut self, bus: &mut dyn Bus) -> R<()> {
        self.steps += 1;
        let at = self.pc;
        self.recent.rotate_left(1);
        self.recent[11] = at;
        let op = self.fetch(bus)?;
        let r = self.exec(bus, op);
        r.map_err(|e| format!("{e} [opcode {op:#06x} at {at:#x}]"))
    }

    fn exec(&mut self, bus: &mut dyn Bus, op: u16) -> R<()> {
        let mode = op >> 3 & 7;
        let reg = op & 7;
        let rn = usize::from(op >> 9 & 7);
        let ssz = Sz::from_bits(op >> 6 & 3);
        match op >> 12 {
            0x0 => {
                if op & 0x138 == 0x108 {
                    return Err("movep isn't supported".into());
                }
                if op & 0x100 != 0 || op >> 8 & 0xf == 8 {
                    // bit operations
                    let bit = if op & 0x100 != 0 {
                        self.d[rn]
                    } else {
                        u32::from(self.fetch(bus)?)
                    };
                    let l = self.loc(bus, mode, reg, Sz::B)?;
                    let (sz, bit) = if let Loc::D(_) = l { (Sz::L, bit & 31) } else { (Sz::B, bit & 7) };
                    let v = self.rd(bus, l, sz)?;
                    self.flag(Z, v >> bit & 1 == 0);
                    let nv = match op >> 6 & 3 {
                        0 => return Ok(()),
                        1 => v ^ 1 << bit,
                        2 => v & !(1 << bit),
                        _ => v | 1 << bit,
                    };
                    return self.wr(bus, l, sz, nv);
                }
                let sz = ssz.ok_or("bad size")?;
                let imm = if sz == Sz::B {
                    u32::from(self.fetch(bus)? & 0xff)
                } else if sz == Sz::W {
                    u32::from(self.fetch(bus)?)
                } else {
                    self.fetch32(bus)?
                };
                let kind = op >> 9 & 7;
                if mode == 7 && reg == 4 {
                    // to CCR or SR
                    let m = if sz == Sz::B { 0x1f } else { 0xffff };
                    let new = match kind {
                        0 => self.sr | imm as u16,
                        1 => self.sr & imm as u16,
                        5 => self.sr ^ imm as u16,
                        _ => return Err("bad immediate to sr".into()),
                    } & m;
                    self.sr = if sz == Sz::B { self.sr & 0xff00 | new } else { new };
                    return Ok(());
                }
                let l = self.loc(bus, mode, reg, sz)?;
                let d = self.rd(bus, l, sz)?;
                let res = match kind {
                    0 => {
                        let r = d | imm;
                        self.logic_flags(r, sz);
                        r
                    }
                    1 => {
                        let r = d & imm;
                        self.logic_flags(r, sz);
                        r
                    }
                    2 => self.sub(d, imm, sz, 0, false, true),
                    3 => self.add(d, imm, sz, 0, false, true),
                    5 => {
                        let r = d ^ imm;
                        self.logic_flags(r, sz);
                        r
                    }
                    6 => {
                        self.sub(d, imm, sz, 0, false, false);
                        return Ok(());
                    }
                    _ => return Err("bad immediate op".into()),
                };
                self.wr(bus, l, sz, res)
            }
            0x1 | 0x2 | 0x3 => {
                let sz = match op >> 12 {
                    1 => Sz::B,
                    3 => Sz::W,
                    _ => Sz::L,
                };
                let s = self.loc(bus, mode, reg, sz)?;
                let v = self.rd(bus, s, sz)?;
                let dm = op >> 6 & 7;
                let dr = op >> 9 & 7;
                if dm == 1 {
                    // movea: no flags, word sign-extended
                    let v = if sz == Sz::W { v as u16 as i16 as i32 as u32 } else { v };
                    self.a[usize::from(dr)] = v;
                    return Ok(());
                }
                let d = self.loc(bus, dm, dr, sz)?;
                self.logic_flags(v, sz);
                self.wr(bus, d, sz, v)
            }
            0x4 => self.misc(bus, op),
            0x5 => {
                if op >> 6 & 3 == 3 {
                    let c = op >> 8 & 15;
                    if mode == 1 {
                        // dbcc
                        let base = self.pc;
                        let d = self.fetch(bus)? as i16 as i32 as u32;
                        if !self.cond(c) {
                            let r = usize::from(reg);
                            let n = (self.d[r] as u16).wrapping_sub(1);
                            self.d[r] = self.d[r] & 0xffff_0000 | u32::from(n);
                            if n != 0xffff {
                                self.pc = base.wrapping_add(d);
                            }
                        }
                        return Ok(());
                    }
                    let l = self.loc(bus, mode, reg, Sz::B)?;
                    let v = if self.cond(c) { 0xff } else { 0 };
                    return self.wr(bus, l, Sz::B, v);
                }
                let sz = ssz.ok_or("bad size")?;
                let n = u32::from(op >> 9 & 7);
                let n = if n == 0 { 8 } else { n };
                let l = self.loc(bus, mode, reg, sz)?;
                if let Loc::A(r) = l {
                    self.a[r] = if op & 0x100 != 0 { self.a[r].wrapping_sub(n) } else { self.a[r].wrapping_add(n) };
                    return Ok(());
                }
                let d = self.rd(bus, l, sz)?;
                let r = if op & 0x100 != 0 { self.sub(d, n, sz, 0, false, true) } else { self.add(d, n, sz, 0, false, true) };
                self.wr(bus, l, sz, r)
            }
            0x6 => {
                let c = op >> 8 & 15;
                let base = self.pc;
                let disp = if op & 0xff == 0 {
                    self.fetch(bus)? as i16 as i32 as u32
                } else {
                    op as u8 as i8 as i32 as u32
                };
                if c == 1 {
                    let ret = self.pc;
                    self.push32(bus, ret)?;
                    self.pc = base.wrapping_add(disp);
                } else if self.cond(c) {
                    self.pc = base.wrapping_add(disp);
                }
                Ok(())
            }
            0x7 => {
                let v = op as u8 as i8 as i32 as u32;
                self.d[rn] = v;
                self.logic_flags(v, Sz::L);
                Ok(())
            }
            0x8 | 0xc => {
                let is_or = op >> 12 == 8;
                if op >> 6 & 3 == 3 {
                    let s = self.loc(bus, mode, reg, Sz::W)?;
                    let sv = self.rd(bus, s, Sz::W)?;
                    let signed = op & 0x100 != 0;
                    if is_or {
                        // divu / divs
                        let dv = self.d[rn];
                        if sv == 0 {
                            return Err("division by zero".into());
                        }
                        if signed {
                            let (a, b) = (dv as i32, sv as u16 as i16 as i32);
                            let q = a.wrapping_div(b);
                            if a == i32::MIN && b == -1 || q > 32767 || q < -32768 {
                                self.flag(V, true);
                                return Ok(());
                            }
                            let r = a.wrapping_rem(b);
                            self.d[rn] = (r as u16 as u32) << 16 | q as u16 as u32;
                            self.flag(N, q < 0);
                            self.flag(Z, q == 0);
                        } else {
                            let q = dv / sv;
                            if q > 0xffff {
                                self.flag(V, true);
                                return Ok(());
                            }
                            let r = dv % sv;
                            self.d[rn] = r << 16 | q;
                            self.flag(N, q & 0x8000 != 0);
                            self.flag(Z, q == 0);
                        }
                        self.flag(V, false);
                        self.flag(C, false);
                    } else {
                        let r = if signed {
                            (self.d[rn] as u16 as i16 as i32).wrapping_mul(sv as u16 as i16 as i32) as u32
                        } else {
                            (self.d[rn] & 0xffff) * sv
                        };
                        self.d[rn] = r;
                        self.logic_flags(r, Sz::L);
                    }
                    return Ok(());
                }
                if !is_or && op & 0x130 == 0x100 {
                    // exg
                    let (a, b) = (rn, usize::from(reg));
                    match op & 0xf8 {
                        0x40 => self.d.swap(a, b),
                        0x48 => self.a.swap(a, b),
                        0x88 => std::mem::swap(&mut self.d[a], &mut self.a[b]),
                        _ => return Err("bad exg".into()),
                    }
                    return Ok(());
                }
                if op & 0x1f0 == 0x100 {
                    return Err("bcd instructions aren't supported".into());
                }
                let sz = ssz.ok_or("bad size")?;
                let l = self.loc(bus, mode, reg, sz)?;
                let s = self.rd(bus, l, sz)?;
                if op & 0x100 != 0 {
                    let d = self.d[rn] & sz.mask();
                    let r = if is_or { s | d } else { s & d };
                    self.logic_flags(r, sz);
                    self.wr(bus, l, sz, r)
                } else {
                    let d = self.d[rn] & sz.mask();
                    let r = if is_or { s | d } else { s & d };
                    self.logic_flags(r, sz);
                    self.wr(bus, Loc::D(rn), sz, r)
                }
            }
            0x9 | 0xd => {
                let add = op >> 12 == 0xd;
                if op >> 6 & 3 == 3 {
                    let sz = if op & 0x100 != 0 { Sz::L } else { Sz::W };
                    let l = self.loc(bus, mode, reg, sz)?;
                    let mut s = self.rd(bus, l, sz)?;
                    if sz == Sz::W {
                        s = s as u16 as i16 as i32 as u32;
                    }
                    self.a[rn] = if add { self.a[rn].wrapping_add(s) } else { self.a[rn].wrapping_sub(s) };
                    return Ok(());
                }
                let sz = ssz.ok_or("bad size")?;
                if op & 0x130 == 0x100 {
                    // addx / subx
                    let x = u32::from(self.sr & X != 0);
                    let (sl, dl) = if op & 8 != 0 {
                        let s = self.loc(bus, 4, reg, sz)?;
                        let d = self.loc(bus, 4, op >> 9 & 7, sz)?;
                        (s, d)
                    } else {
                        (Loc::D(usize::from(reg)), Loc::D(rn))
                    };
                    let s = self.rd(bus, sl, sz)?;
                    let d = self.rd(bus, dl, sz)?;
                    let r = if add { self.add(d, s, sz, x, true, true) } else { self.sub(d, s, sz, x, true, true) };
                    return self.wr(bus, dl, sz, r);
                }
                let l = self.loc(bus, mode, reg, sz)?;
                let ev = self.rd(bus, l, sz)?;
                if op & 0x100 != 0 {
                    let dv = self.d[rn] & sz.mask();
                    let r = if add { self.add(ev, dv, sz, 0, false, true) } else { self.sub(ev, dv, sz, 0, false, true) };
                    self.wr(bus, l, sz, r)
                } else {
                    let dv = self.d[rn] & sz.mask();
                    let r = if add { self.add(dv, ev, sz, 0, false, true) } else { self.sub(dv, ev, sz, 0, false, true) };
                    self.wr(bus, Loc::D(rn), sz, r)
                }
            }
            0xb => {
                if op >> 6 & 3 == 3 {
                    let sz = if op & 0x100 != 0 { Sz::L } else { Sz::W };
                    let l = self.loc(bus, mode, reg, sz)?;
                    let mut s = self.rd(bus, l, sz)?;
                    if sz == Sz::W {
                        s = s as u16 as i16 as i32 as u32;
                    }
                    let a = self.a[rn];
                    self.sub(a, s, Sz::L, 0, false, false);
                    return Ok(());
                }
                let sz = ssz.ok_or("bad size")?;
                if op & 0x100 != 0 {
                    if mode == 1 {
                        // cmpm
                        let s = self.loc(bus, 3, reg, sz)?;
                        let sv = self.rd(bus, s, sz)?;
                        let d = self.loc(bus, 3, op >> 9 & 7, sz)?;
                        let dv = self.rd(bus, d, sz)?;
                        self.sub(dv, sv, sz, 0, false, false);
                        return Ok(());
                    }
                    // eor
                    let l = self.loc(bus, mode, reg, sz)?;
                    let e = self.rd(bus, l, sz)?;
                    let r = e ^ (self.d[rn] & sz.mask());
                    self.logic_flags(r, sz);
                    return self.wr(bus, l, sz, r);
                }
                let l = self.loc(bus, mode, reg, sz)?;
                let s = self.rd(bus, l, sz)?;
                let d = self.d[rn] & sz.mask();
                self.sub(d, s, sz, 0, false, false);
                Ok(())
            }
            0xe => self.shift(bus, op),
            _ => Err("line A/F or unknown opcode".into()),
        }
    }

    fn misc(&mut self, bus: &mut dyn Bus, op: u16) -> R<()> {
        let mode = op >> 3 & 7;
        let reg = op & 7;
        let rn = usize::from(op >> 9 & 7);
        match op {
            0x4e71 => return Ok(()),
            0x4e75 => {
                self.pc = self.mread(bus, self.a[7], Sz::L)?;
                self.a[7] = self.a[7].wrapping_add(4);
                return Ok(());
            }
            0x4e73 => {
                self.sr = self.r16(bus, self.a[7])?;
                self.pc = self.mread(bus, self.a[7].wrapping_add(2), Sz::L)?;
                self.a[7] = self.a[7].wrapping_add(6);
                return Ok(());
            }
            0x4e70 => return Ok(()),
            _ => {}
        }
        if op & 0x1c0 == 0x1c0 {
            // lea
            let l = self.loc(bus, mode, reg, Sz::L)?;
            if let Loc::M(a) = l {
                self.a[rn] = a;
                return Ok(());
            }
            return Err("lea of a non-address".into());
        }
        if op & 0xfff8 == 0x4e50 {
            let r = usize::from(reg);
            let v = self.a[r];
            self.push32(bus, v)?;
            self.a[r] = self.a[7];
            let d = self.fetch(bus)? as i16 as i32 as u32;
            self.a[7] = self.a[7].wrapping_add(d);
            return Ok(());
        }
        if op & 0xfff8 == 0x4e58 {
            let r = usize::from(reg);
            self.a[7] = self.a[r];
            self.a[r] = self.mread(bus, self.a[7], Sz::L)?;
            self.a[7] = self.a[7].wrapping_add(4);
            return Ok(());
        }
        if op & 0xffc0 == 0x4ec0 || op & 0xffc0 == 0x4e80 {
            let l = self.loc(bus, mode, reg, Sz::L)?;
            let Loc::M(a) = l else { return Err("jump to a non-address".into()) };
            if op & 0x40 == 0 {
                let ret = self.pc;
                self.push32(bus, ret)?;
            }
            self.pc = a;
            return Ok(());
        }
        match op & 0xffc0 {
            0x40c0 => {
                let l = self.loc(bus, mode, reg, Sz::W)?;
                let v = u32::from(self.sr);
                return self.wr(bus, l, Sz::W, v);
            }
            0x44c0 => {
                let l = self.loc(bus, mode, reg, Sz::W)?;
                let v = self.rd(bus, l, Sz::W)? as u16;
                self.sr = self.sr & 0xff00 | v & 0x1f;
                return Ok(());
            }
            0x46c0 => {
                let l = self.loc(bus, mode, reg, Sz::W)?;
                self.sr = self.rd(bus, l, Sz::W)? as u16;
                return Ok(());
            }
            0x4840 => {
                if mode == 0 {
                    let r = usize::from(reg);
                    self.d[r] = self.d[r].rotate_left(16);
                    let v = self.d[r];
                    self.logic_flags(v, Sz::L);
                    return Ok(());
                }
                let l = self.loc(bus, mode, reg, Sz::L)?;
                if let Loc::M(a) = l {
                    return self.push32(bus, a);
                }
                return Err("pea of a non-address".into());
            }
            _ => {}
        }
        if op & 0xfb80 == 0x4880 {
            // ext / movem
            if mode == 0 {
                let r = usize::from(reg);
                if op & 0x40 == 0 {
                    let v = self.d[r] as u8 as i8 as i16 as u16;
                    self.d[r] = self.d[r] & 0xffff_0000 | u32::from(v);
                    self.logic_flags(u32::from(v), Sz::W);
                } else {
                    self.d[r] = self.d[r] as u16 as i16 as i32 as u32;
                    let v = self.d[r];
                    self.logic_flags(v, Sz::L);
                }
                return Ok(());
            }
            let sz = if op & 0x40 != 0 { Sz::L } else { Sz::W };
            let mask = self.fetch(bus)?;
            let to_reg = op & 0x400 != 0;
            let n = sz.bytes();
            if to_reg {
                let mut a = match mode {
                    3 => self.a[usize::from(reg)],
                    _ => match self.loc(bus, mode, reg, sz)? {
                        Loc::M(a) => a,
                        _ => return Err("bad movem".into()),
                    },
                };
                for i in 0..16 {
                    if mask >> i & 1 != 0 {
                        let mut v = self.mread(bus, a, sz)?;
                        if sz == Sz::W {
                            v = v as u16 as i16 as i32 as u32;
                        }
                        if i < 8 {
                            self.d[i] = v;
                        } else {
                            self.a[i - 8] = v;
                        }
                        a = a.wrapping_add(n);
                    }
                }
                if mode == 3 {
                    self.a[usize::from(reg)] = a;
                }
            } else if mode == 4 {
                let mut a = self.a[usize::from(reg)];
                for i in (0..16).rev() {
                    // in predecrement the mask is reversed: bit 0 is a7
                    if mask >> (15 - i) & 1 != 0 {
                        a = a.wrapping_sub(n);
                        let v = if i < 8 { self.d[i] } else { self.a[i - 8] };
                        self.mwrite(bus, a, sz, v)?;
                    }
                }
                self.a[usize::from(reg)] = a;
            } else {
                let Loc::M(mut a) = self.loc(bus, mode, reg, sz)? else { return Err("bad movem".into()) };
                for i in 0..16 {
                    if mask >> i & 1 != 0 {
                        let v = if i < 8 { self.d[i] } else { self.a[i - 8] };
                        self.mwrite(bus, a, sz, v)?;
                        a = a.wrapping_add(n);
                    }
                }
            }
            return Ok(());
        }
        // clr, neg, negx, not, tst
        let sz = Sz::from_bits(op >> 6 & 3).ok_or("bad size")?;
        let kind = op >> 8 & 0xf;
        if matches!(kind, 0x0 | 0x2 | 0x4 | 0x6 | 0xa) {
            let l = self.loc(bus, mode, reg, sz)?;
            match kind {
                0x2 => {
                    self.flag(N, false);
                    self.flag(Z, true);
                    self.flag(V, false);
                    self.flag(C, false);
                    return self.wr(bus, l, sz, 0);
                }
                0xa => {
                    let v = self.rd(bus, l, sz)?;
                    self.logic_flags(v, sz);
                    return Ok(());
                }
                0x6 => {
                    let v = !self.rd(bus, l, sz)? & sz.mask();
                    self.logic_flags(v, sz);
                    return self.wr(bus, l, sz, v);
                }
                0x4 => {
                    let v = self.rd(bus, l, sz)?;
                    let r = self.sub(0, v, sz, 0, false, true);
                    return self.wr(bus, l, sz, r);
                }
                _ => {
                    let v = self.rd(bus, l, sz)?;
                    let x = u32::from(self.sr & X != 0);
                    let r = self.sub(0, v, sz, x, true, true);
                    return self.wr(bus, l, sz, r);
                }
            }
        }
        Err("unsupported instruction".into())
    }

    fn shift(&mut self, bus: &mut dyn Bus, op: u16) -> R<()> {
        let left = op & 0x100 != 0;
        if op >> 6 & 3 == 3 {
            // memory: one bit, a word
            let kind = op >> 9 & 7;
            let l = self.loc(bus, op >> 3 & 7, op & 7, Sz::W)?;
            let v = self.rd(bus, l, Sz::W)?;
            let r = self.shift_op(kind, left, v, 1, Sz::W);
            return self.wr(bus, l, Sz::W, r);
        }
        let sz = Sz::from_bits(op >> 6 & 3).ok_or("bad size")?;
        let kind = op >> 3 & 3;
        let rg = usize::from(op & 7);
        let cnt = if op & 0x20 != 0 {
            self.d[usize::from(op >> 9 & 7)] & 63
        } else {
            let n = u32::from(op >> 9 & 7);
            if n == 0 {
                8
            } else {
                n
            }
        };
        let v = self.d[rg] & sz.mask();
        let r = self.shift_op(kind, left, v, cnt, sz);
        self.wr(bus, Loc::D(rg), sz, r)
    }

    /// kind: 0 arithmetic, 1 logical, 2 rotate through X, 3 rotate.
    fn shift_op(&mut self, kind: u16, left: bool, v: u32, cnt: u32, sz: Sz) -> u32 {
        let bits = sz.bytes() * 8;
        let mut v = v & sz.mask();
        let mut c = false;
        let mut ovf = false;
        let mut x = self.sr & X != 0;
        if cnt == 0 {
            self.logic_flags(v, sz);
            self.flag(C, kind == 2 && x);
            return v;
        }
        for _ in 0..cnt {
            match (kind, left) {
                (0, true) | (1, true) => {
                    c = v & sz.msb() != 0;
                    let nv = v << 1 & sz.mask();
                    if kind == 0 && (nv ^ v) & sz.msb() != 0 {
                        ovf = true;
                    }
                    v = nv;
                    x = c;
                }
                (0, false) => {
                    c = v & 1 != 0;
                    v = v >> 1 | v & sz.msb();
                    x = c;
                }
                (1, false) => {
                    c = v & 1 != 0;
                    v >>= 1;
                    x = c;
                }
                (2, true) => {
                    c = v & sz.msb() != 0;
                    v = (v << 1 & sz.mask()) | u32::from(x);
                    x = c;
                }
                (2, false) => {
                    c = v & 1 != 0;
                    v = v >> 1 | if x { sz.msb() } else { 0 };
                    x = c;
                }
                (_, true) => {
                    c = v & sz.msb() != 0;
                    v = (v << 1 & sz.mask()) | u32::from(c);
                }
                (_, false) => {
                    c = v & 1 != 0;
                    v = v >> 1 | if c { sz.msb() } else { 0 };
                }
            }
        }
        let _ = bits;
        self.nz(v, sz);
        self.flag(V, ovf);
        self.flag(C, c);
        if kind != 3 {
            self.flag(X, x);
        }
        v
    }
}
