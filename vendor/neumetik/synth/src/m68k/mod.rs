//! A 68000 assembler (the dialect MDSDRV's source is written in) and interpreter, to run the original Mega Drive sound driver (`mdsdrv`'s tests).

pub mod asm;
pub mod cpu;

#[cfg(test)]
pub mod mds_ref;

#[cfg(test)]
mod tests {
    use super::asm::assemble;
    use super::cpu::{Bus, Cpu};

    struct Ram(Vec<u8>);
    impl Bus for Ram {
        fn read8(&mut self, a: u32) -> u8 {
            self.0.get(a as usize).copied().unwrap_or(0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            if let Some(m) = self.0.get_mut(a as usize) {
                *m = v;
            }
        }
    }

    fn asm(src: &str) -> super::asm::Output {
        let s = src.to_string();
        assemble("t.68k", &[], &move |n| (n == "t.68k").then(|| s.clone().into_bytes())).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn instructions_have_the_known_encodings() {
        let o = asm("\tmoveq\t#1,d0\n\tmove.w\td1,-(sp)\n\tlea\t4(a0),a1\n\trts\n\tbra.w\t*\n\tmovem.l\td0-d1/a2,-(sp)\n\tadd.b\td0,d1\n\taddq.b\t#1,d0\n\tbtst\t#0,(a3)\n\tmove.b\t(sp)+,(a1)+\n\tdbra\td0,*\n");
        let w: Vec<u16> = o.bytes.chunks(2).map(|c| u16::from(c[0]) << 8 | u16::from(c[1])).collect();
        assert_eq!(
            w,
            // moveq, move.w d1,-(sp), lea 4(a0),a1 (+ext), rts, bra.w (+ext -2... to itself), movem (+mask: a2,d1,d0 reversed), add.b, addq.b, btst (+bit), move.b, dbra (+ext)
            [0x7001, 0x3f01, 0x43e8, 0x0004, 0x4e75, 0x6000, 0xfffe, 0x48e7, 0xc020, 0xd200, 0x5200, 0x0813, 0x0000, 0x12df, 0x51c8, 0xfffe],
            "{w:04x?}"
        );
    }

    #[test]
    fn locals_macros_and_conditionals_assemble_and_run() {
        let o = asm(concat!(
            "N\tequ\t5\n",
            "count\tequr\td3\n",
            "twice\tmacro\tr\n\tadd.w\t\\r,\\r\n\tendm\n",
            "sum\n\tmoveq\t#0,d0\n\tmoveq\t#N-1,count\n",
            "@next\n\tadd.w\tcount,d0\n\tdbra\tcount,@next\n",
            "\ttwice\td0\n",
            "\tif N>3\n\taddq.w\t#1,d0\n\telse\n\taddq.w\t#2,d0\n\tendif\n",
            "\trept\t2\n\tnop\n\tendr\n\trts\n",
        ));
        let mut ram = Ram(vec![0; 0x10000]);
        ram.0[..o.bytes.len()].copy_from_slice(&o.bytes);
        let mut c = Cpu::default();
        c.a[7] = 0x8000;
        c.call(&mut ram, 0, 1000).unwrap();
        // (4+3+2+1+0) * 2 + 1
        assert_eq!(c.d[0] & 0xffff, 21);
    }

    #[test]
    fn arithmetic_sets_the_flags_the_branches_read() {
        let o = asm("\tmoveq\t#-1,d0\n\taddq.b\t#1,d0\n\tbne.s\tbad\n\tbcc.s\tbad\n\tmoveq\t#100,d1\n\tadd.b\t#100,d1\n\tbvc.s\tbad\n\tmoveq\t#7,d2\n\trts\nbad\n\tmoveq\t#0,d2\n\trts\n");
        let mut ram = Ram(vec![0; 0x10000]);
        ram.0[..o.bytes.len()].copy_from_slice(&o.bytes);
        let mut c = Cpu::default();
        c.a[7] = 0x8000;
        c.call(&mut ram, 0, 1000).unwrap();
        assert_eq!(c.d[2], 7, "carry and zero from 255+1 in a byte, overflow from 100+100");
    }
}
