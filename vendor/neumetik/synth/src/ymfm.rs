// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from ymfm (github.com/aaronsgiles/ymfm), under its BSD 3-Clause
// License:
//
// Copyright (c) 2021, Aaron Giles
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this
//    list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its
//    contributors may be used to endorse or promote products derived from
//    this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! Yamaha's 4-operator FM chips as ymfm models them: the engine they share
//! (operators, envelopes with SSG-EG, the LFO, the algorithms), the OPN
//! family's registers (the YM2612, OPN2, and the YM2610's FM, OPNB) and the
//! OPM's (the YM2151, with its noise and LFO waveforms). Timers, status
//! and IRQs, which only a sound driver on the chip needs, are left out;
//! and a key turned off and on again between two samples sounds again, as
//! it does on the chip (ymfm, taking writes at once, would hold it).

fn bitfield(v: u32, start: u32, len: u32) -> u32 {
    (v >> start) & ((1 << len) - 1)
}

const EG_ATTACK: usize = 1;
const EG_DECAY: usize = 2;
const EG_SUSTAIN: usize = 3;
const EG_RELEASE: usize = 4;
const EG_STATES: usize = 6;
const EG_QUIET: u16 = 0x380;
const PHASE_STEP_DYNAMIC: u32 = 1;
const WAVEFORM_LENGTH: usize = 0x400;

/// |sin| of a 10-bit phase as a 4.8 attenuation (the OPN's own table).
fn abs_sin_attenuation(input: u32) -> u32 {
    const T: [u16; 256] = [
        0x859, 0x6c3, 0x607, 0x58b, 0x52e, 0x4e4, 0x4a6, 0x471, 0x443, 0x41a, 0x3f5, 0x3d3, 0x3b5, 0x398, 0x37e, 0x365, 0x34e, 0x339, 0x324, 0x311, 0x2ff, 0x2ed, 0x2dc, 0x2cd, 0x2bd, 0x2af, 0x2a0,
        0x293, 0x286, 0x279, 0x26d, 0x261, 0x256, 0x24b, 0x240, 0x236, 0x22c, 0x222, 0x218, 0x20f, 0x206, 0x1fd, 0x1f5, 0x1ec, 0x1e4, 0x1dc, 0x1d4, 0x1cd, 0x1c5, 0x1be, 0x1b7, 0x1b0, 0x1a9, 0x1a2,
        0x19b, 0x195, 0x18f, 0x188, 0x182, 0x17c, 0x177, 0x171, 0x16b, 0x166, 0x160, 0x15b, 0x155, 0x150, 0x14b, 0x146, 0x141, 0x13c, 0x137, 0x133, 0x12e, 0x129, 0x125, 0x121, 0x11c, 0x118, 0x114,
        0x10f, 0x10b, 0x107, 0x103, 0x0ff, 0x0fb, 0x0f8, 0x0f4, 0x0f0, 0x0ec, 0x0e9, 0x0e5, 0x0e2, 0x0de, 0x0db, 0x0d7, 0x0d4, 0x0d1, 0x0cd, 0x0ca, 0x0c7, 0x0c4, 0x0c1, 0x0be, 0x0bb, 0x0b8, 0x0b5,
        0x0b2, 0x0af, 0x0ac, 0x0a9, 0x0a7, 0x0a4, 0x0a1, 0x09f, 0x09c, 0x099, 0x097, 0x094, 0x092, 0x08f, 0x08d, 0x08a, 0x088, 0x086, 0x083, 0x081, 0x07f, 0x07d, 0x07a, 0x078, 0x076, 0x074, 0x072,
        0x070, 0x06e, 0x06c, 0x06a, 0x068, 0x066, 0x064, 0x062, 0x060, 0x05e, 0x05c, 0x05b, 0x059, 0x057, 0x055, 0x053, 0x052, 0x050, 0x04e, 0x04d, 0x04b, 0x04a, 0x048, 0x046, 0x045, 0x043, 0x042,
        0x040, 0x03f, 0x03e, 0x03c, 0x03b, 0x039, 0x038, 0x037, 0x035, 0x034, 0x033, 0x031, 0x030, 0x02f, 0x02e, 0x02d, 0x02b, 0x02a, 0x029, 0x028, 0x027, 0x026, 0x025, 0x024, 0x023, 0x022, 0x021,
        0x020, 0x01f, 0x01e, 0x01d, 0x01c, 0x01b, 0x01a, 0x019, 0x018, 0x017, 0x017, 0x016, 0x015, 0x014, 0x014, 0x013, 0x012, 0x011, 0x011, 0x010, 0x00f, 0x00f, 0x00e, 0x00d, 0x00d, 0x00c, 0x00c,
        0x00b, 0x00a, 0x00a, 0x009, 0x009, 0x008, 0x008, 0x007, 0x007, 0x007, 0x006, 0x006, 0x005, 0x005, 0x005, 0x004, 0x004, 0x004, 0x003, 0x003, 0x003, 0x002, 0x002, 0x002, 0x002, 0x001, 0x001,
        0x001, 0x001, 0x001, 0x001, 0x001, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    ];
    let i = if bitfield(input, 8, 1) != 0 { !input } else { input };
    u32::from(T[(i & 0xff) as usize])
}

/// A 5.8 attenuation as a 13-bit linear volume (the OPN's power table).
fn attenuation_to_volume(input: u32) -> u32 {
    const M: [u16; 256] = [
        0x3fa, 0x3f5, 0x3ef, 0x3ea, 0x3e4, 0x3df, 0x3da, 0x3d4, 0x3cf, 0x3c9, 0x3c4, 0x3bf, 0x3b9, 0x3b4, 0x3ae, 0x3a9, 0x3a4, 0x39f, 0x399, 0x394, 0x38f, 0x38a, 0x384, 0x37f, 0x37a, 0x375, 0x370,
        0x36a, 0x365, 0x360, 0x35b, 0x356, 0x351, 0x34c, 0x347, 0x342, 0x33d, 0x338, 0x333, 0x32e, 0x329, 0x324, 0x31f, 0x31a, 0x315, 0x310, 0x30b, 0x306, 0x302, 0x2fd, 0x2f8, 0x2f3, 0x2ee, 0x2e9,
        0x2e5, 0x2e0, 0x2db, 0x2d6, 0x2d2, 0x2cd, 0x2c8, 0x2c4, 0x2bf, 0x2ba, 0x2b5, 0x2b1, 0x2ac, 0x2a8, 0x2a3, 0x29e, 0x29a, 0x295, 0x291, 0x28c, 0x288, 0x283, 0x27f, 0x27a, 0x276, 0x271, 0x26d,
        0x268, 0x264, 0x25f, 0x25b, 0x257, 0x252, 0x24e, 0x249, 0x245, 0x241, 0x23c, 0x238, 0x234, 0x230, 0x22b, 0x227, 0x223, 0x21e, 0x21a, 0x216, 0x212, 0x20e, 0x209, 0x205, 0x201, 0x1fd, 0x1f9,
        0x1f5, 0x1f0, 0x1ec, 0x1e8, 0x1e4, 0x1e0, 0x1dc, 0x1d8, 0x1d4, 0x1d0, 0x1cc, 0x1c8, 0x1c4, 0x1c0, 0x1bc, 0x1b8, 0x1b4, 0x1b0, 0x1ac, 0x1a8, 0x1a4, 0x1a0, 0x19c, 0x199, 0x195, 0x191, 0x18d,
        0x189, 0x185, 0x181, 0x17e, 0x17a, 0x176, 0x172, 0x16f, 0x16b, 0x167, 0x163, 0x160, 0x15c, 0x158, 0x154, 0x151, 0x14d, 0x149, 0x146, 0x142, 0x13e, 0x13b, 0x137, 0x134, 0x130, 0x12c, 0x129,
        0x125, 0x122, 0x11e, 0x11b, 0x117, 0x114, 0x110, 0x10c, 0x109, 0x106, 0x102, 0x0ff, 0x0fb, 0x0f8, 0x0f4, 0x0f1, 0x0ed, 0x0ea, 0x0e7, 0x0e3, 0x0e0, 0x0dc, 0x0d9, 0x0d6, 0x0d2, 0x0cf, 0x0cc,
        0x0c8, 0x0c5, 0x0c2, 0x0be, 0x0bb, 0x0b8, 0x0b5, 0x0b1, 0x0ae, 0x0ab, 0x0a8, 0x0a4, 0x0a1, 0x09e, 0x09b, 0x098, 0x094, 0x091, 0x08e, 0x08b, 0x088, 0x085, 0x082, 0x07e, 0x07b, 0x078, 0x075,
        0x072, 0x06f, 0x06c, 0x069, 0x066, 0x063, 0x060, 0x05d, 0x05a, 0x057, 0x054, 0x051, 0x04e, 0x04b, 0x048, 0x045, 0x042, 0x03f, 0x03c, 0x039, 0x036, 0x033, 0x030, 0x02d, 0x02a, 0x028, 0x025,
        0x022, 0x01f, 0x01c, 0x019, 0x016, 0x014, 0x011, 0x00e, 0x00b, 0x008, 0x006, 0x003, 0x000,
    ];
    ((u32::from(M[(input & 0xff) as usize]) | 0x400) << 2) >> (input >> 8)
}

/// The 4-bit envelope increment for a rate and step.
fn attenuation_increment(rate: u32, index: u32) -> u32 {
    const T: [u32; 64] = [
        0x00000000, 0x00000000, 0x10101010, 0x10101010, 0x10101010, 0x10101010, 0x11101110, 0x11101110, 0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110,
        0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110,
        0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110, 0x10101010, 0x10111010, 0x11101110, 0x11111110,
        0x11111111, 0x21112111, 0x21212121, 0x22212221, 0x22222222, 0x42224222, 0x42424242, 0x44424442, 0x44444444, 0x84448444, 0x84848484, 0x88848884, 0x88888888, 0x88888888, 0x88888888, 0x88888888,
    ];
    bitfield(T[rate as usize], 4 * index, 4)
}

fn detune_adjustment(detune: u32, keycode: u32) -> i32 {
    const T: [[u8; 4]; 32] = [
        [0, 0, 1, 2],
        [0, 0, 1, 2],
        [0, 0, 1, 2],
        [0, 0, 1, 2],
        [0, 1, 2, 2],
        [0, 1, 2, 3],
        [0, 1, 2, 3],
        [0, 1, 2, 3],
        [0, 1, 2, 4],
        [0, 1, 3, 4],
        [0, 1, 3, 4],
        [0, 1, 3, 5],
        [0, 2, 4, 5],
        [0, 2, 4, 6],
        [0, 2, 4, 6],
        [0, 2, 5, 7],
        [0, 2, 5, 8],
        [0, 3, 6, 8],
        [0, 3, 6, 9],
        [0, 3, 7, 10],
        [0, 4, 8, 11],
        [0, 4, 8, 12],
        [0, 4, 9, 13],
        [0, 5, 10, 14],
        [0, 5, 11, 16],
        [0, 6, 12, 17],
        [0, 6, 13, 19],
        [0, 7, 14, 20],
        [0, 8, 16, 22],
        [0, 8, 16, 22],
        [0, 8, 16, 22],
        [0, 8, 16, 22],
    ];
    let r = i32::from(T[keycode as usize][(detune & 3) as usize]);
    if bitfield(detune, 2, 1) != 0 {
        -r
    } else {
        r
    }
}

/// The OPM's phase step for its block, key code and fraction, after a delta.
fn opm_key_code_to_phase_step(block_freq: u32, delta: i32) -> u32 {
    const S: [u32; 768] = [
        41568, 41600, 41632, 41664, 41696, 41728, 41760, 41792, 41856, 41888, 41920, 41952, 42016, 42048, 42080, 42112, 42176, 42208, 42240, 42272, 42304, 42336, 42368, 42400, 42464, 42496, 42528,
        42560, 42624, 42656, 42688, 42720, 42784, 42816, 42848, 42880, 42912, 42944, 42976, 43008, 43072, 43104, 43136, 43168, 43232, 43264, 43296, 43328, 43392, 43424, 43456, 43488, 43552, 43584,
        43616, 43648, 43712, 43744, 43776, 43808, 43872, 43904, 43936, 43968, 44032, 44064, 44096, 44128, 44192, 44224, 44256, 44288, 44352, 44384, 44416, 44448, 44512, 44544, 44576, 44608, 44672,
        44704, 44736, 44768, 44832, 44864, 44896, 44928, 44992, 45024, 45056, 45088, 45152, 45184, 45216, 45248, 45312, 45344, 45376, 45408, 45472, 45504, 45536, 45568, 45632, 45664, 45728, 45760,
        45792, 45824, 45888, 45920, 45984, 46016, 46048, 46080, 46144, 46176, 46208, 46240, 46304, 46336, 46368, 46400, 46464, 46496, 46528, 46560, 46656, 46688, 46720, 46752, 46816, 46848, 46880,
        46912, 46976, 47008, 47072, 47104, 47136, 47168, 47232, 47264, 47328, 47360, 47392, 47424, 47488, 47520, 47552, 47584, 47648, 47680, 47744, 47776, 47808, 47840, 47904, 47936, 48032, 48064,
        48096, 48128, 48192, 48224, 48288, 48320, 48384, 48416, 48448, 48480, 48544, 48576, 48640, 48672, 48736, 48768, 48800, 48832, 48896, 48928, 48992, 49024, 49088, 49120, 49152, 49184, 49248,
        49280, 49344, 49376, 49440, 49472, 49504, 49536, 49600, 49632, 49696, 49728, 49792, 49824, 49856, 49888, 49952, 49984, 50048, 50080, 50144, 50176, 50208, 50240, 50304, 50336, 50400, 50432,
        50496, 50528, 50560, 50592, 50656, 50688, 50752, 50784, 50880, 50912, 50944, 50976, 51040, 51072, 51136, 51168, 51232, 51264, 51328, 51360, 51424, 51456, 51488, 51520, 51616, 51648, 51680,
        51712, 51776, 51808, 51872, 51904, 51968, 52000, 52064, 52096, 52160, 52192, 52224, 52256, 52384, 52416, 52448, 52480, 52544, 52576, 52640, 52672, 52736, 52768, 52832, 52864, 52928, 52960,
        52992, 53024, 53120, 53152, 53216, 53248, 53312, 53344, 53408, 53440, 53504, 53536, 53600, 53632, 53696, 53728, 53792, 53824, 53920, 53952, 54016, 54048, 54112, 54144, 54208, 54240, 54304,
        54336, 54400, 54432, 54496, 54528, 54592, 54624, 54688, 54720, 54784, 54816, 54880, 54912, 54976, 55008, 55072, 55104, 55168, 55200, 55264, 55296, 55360, 55392, 55488, 55520, 55584, 55616,
        55680, 55712, 55776, 55808, 55872, 55936, 55968, 56032, 56064, 56128, 56160, 56224, 56288, 56320, 56384, 56416, 56480, 56512, 56576, 56608, 56672, 56736, 56768, 56832, 56864, 56928, 56960,
        57024, 57120, 57152, 57216, 57248, 57312, 57376, 57408, 57472, 57536, 57568, 57632, 57664, 57728, 57792, 57824, 57888, 57952, 57984, 58048, 58080, 58144, 58208, 58240, 58304, 58368, 58400,
        58464, 58496, 58560, 58624, 58656, 58720, 58784, 58816, 58880, 58912, 58976, 59040, 59072, 59136, 59200, 59232, 59296, 59328, 59392, 59456, 59488, 59552, 59648, 59680, 59744, 59776, 59840,
        59904, 59936, 60000, 60064, 60128, 60160, 60224, 60288, 60320, 60384, 60416, 60512, 60544, 60608, 60640, 60704, 60768, 60800, 60864, 60928, 60992, 61024, 61088, 61152, 61184, 61248, 61280,
        61376, 61408, 61472, 61536, 61600, 61632, 61696, 61760, 61824, 61856, 61920, 61984, 62048, 62080, 62144, 62208, 62272, 62304, 62368, 62432, 62496, 62528, 62592, 62656, 62720, 62752, 62816,
        62880, 62944, 62976, 63040, 63104, 63200, 63232, 63296, 63360, 63424, 63456, 63520, 63584, 63648, 63680, 63744, 63808, 63872, 63904, 63968, 64032, 64096, 64128, 64192, 64256, 64320, 64352,
        64416, 64480, 64544, 64608, 64672, 64704, 64768, 64832, 64896, 64928, 65024, 65056, 65120, 65184, 65248, 65312, 65376, 65408, 65504, 65536, 65600, 65664, 65728, 65792, 65856, 65888, 65984,
        66016, 66080, 66144, 66208, 66272, 66336, 66368, 66464, 66496, 66560, 66624, 66688, 66752, 66816, 66848, 66944, 66976, 67040, 67104, 67168, 67232, 67296, 67328, 67424, 67456, 67520, 67584,
        67648, 67712, 67776, 67808, 67904, 67936, 68000, 68064, 68128, 68192, 68256, 68288, 68384, 68448, 68512, 68544, 68640, 68672, 68736, 68800, 68896, 68928, 68992, 69056, 69120, 69184, 69248,
        69280, 69376, 69440, 69504, 69536, 69632, 69664, 69728, 69792, 69920, 69952, 70016, 70080, 70144, 70208, 70272, 70304, 70400, 70464, 70528, 70560, 70656, 70688, 70752, 70816, 70912, 70976,
        71040, 71104, 71136, 71232, 71264, 71360, 71424, 71488, 71552, 71616, 71648, 71744, 71776, 71872, 71968, 72032, 72096, 72160, 72192, 72288, 72320, 72416, 72480, 72544, 72608, 72672, 72704,
        72800, 72832, 72928, 72992, 73056, 73120, 73184, 73216, 73312, 73344, 73440, 73504, 73568, 73632, 73696, 73728, 73824, 73856, 73952, 74080, 74144, 74208, 74272, 74304, 74400, 74432, 74528,
        74592, 74656, 74720, 74784, 74816, 74912, 74944, 75040, 75136, 75200, 75264, 75328, 75360, 75456, 75488, 75584, 75648, 75712, 75776, 75840, 75872, 75968, 76000, 76096, 76224, 76288, 76352,
        76416, 76448, 76544, 76576, 76672, 76736, 76800, 76864, 76928, 77024, 77120, 77152, 77248, 77344, 77408, 77472, 77536, 77568, 77664, 77696, 77792, 77856, 77920, 77984, 78048, 78144, 78240,
        78272, 78368, 78464, 78528, 78592, 78656, 78688, 78784, 78816, 78912, 78976, 79040, 79104, 79168, 79264, 79360, 79392, 79488, 79616, 79680, 79744, 79808, 79840, 79936, 79968, 80064, 80128,
        80192, 80256, 80320, 80416, 80512, 80544, 80640, 80768, 80832, 80896, 80960, 80992, 81088, 81120, 81216, 81280, 81344, 81408, 81472, 81568, 81664, 81696, 81792, 81952, 82016, 82080, 82144,
        82176, 82272, 82304, 82400, 82464, 82528, 82592, 82656, 82752, 82848, 82880, 82976,
    ];
    let mut block = bitfield(block_freq, 10, 3) as i32;
    let adjusted = bitfield(block_freq, 6, 4) as i32 - bitfield(block_freq, 8, 2) as i32;
    let mut eff = (adjusted << 6) | bitfield(block_freq, 0, 6) as i32;
    eff += delta;
    if !(0..768).contains(&eff) {
        if eff < 0 {
            eff += 768;
            if block == 0 {
                return S[0] >> 7;
            }
            block -= 1;
        } else {
            eff -= 768;
            if eff >= 768 {
                block += 1;
                eff -= 768;
            }
            if block >= 7 {
                return S[767];
            }
            block += 1;
        }
    }
    S[eff as usize] >> (block ^ 7)
}

fn opn_lfo_pm_phase_adjustment(fnum_bits: u32, pm_sensitivity: u32, lfo_raw_pm: i32) -> i32 {
    const SHIFTS: [[u8; 8]; 8] = [
        [0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77],
        [0x77, 0x77, 0x77, 0x77, 0x72, 0x72, 0x72, 0x72],
        [0x77, 0x77, 0x77, 0x72, 0x72, 0x72, 0x17, 0x17],
        [0x77, 0x77, 0x72, 0x72, 0x17, 0x17, 0x12, 0x12],
        [0x77, 0x77, 0x72, 0x17, 0x17, 0x17, 0x12, 0x07],
        [0x77, 0x77, 0x17, 0x12, 0x07, 0x07, 0x02, 0x01],
        [0x77, 0x77, 0x17, 0x12, 0x07, 0x07, 0x02, 0x01],
        [0x77, 0x77, 0x17, 0x12, 0x07, 0x07, 0x02, 0x01],
    ];
    let abs_pm = lfo_raw_pm.unsigned_abs();
    let shifts = u32::from(SHIFTS[pm_sensitivity as usize][bitfield(abs_pm, 0, 3) as usize]);
    let mut adjust = ((fnum_bits >> bitfield(shifts, 0, 4)) + (fnum_bits >> bitfield(shifts, 4, 4))) as i32;
    if pm_sensitivity > 5 {
        adjust <<= pm_sensitivity - 5;
    }
    adjust >>= 2;
    if lfo_raw_pm < 0 {
        -adjust
    } else {
        adjust
    }
}

fn effective_rate(rawrate: u32, ksr: u32) -> u8 {
    if rawrate == 0 {
        0
    } else {
        (rawrate + ksr).min(63) as u8
    }
}

/// An operator's values worked out from the registers.
#[derive(Clone, Copy, Default)]
pub struct OpCache {
    phase_step: u32,
    total_level: u32,
    block_freq: u32,
    detune: i32,
    multiple: u32,
    eg_sustain: u32,
    eg_rate: [u8; EG_STATES],
}

/// A chip family's registers, as the engine reads them.
pub trait Regs {
    const CHANNELS: usize;
    const EG_HAS_SSG: bool;
    fn channel_offset(ch: usize) -> u32;
    fn operator_offset(op: usize) -> u32;
    /// Each channel's four operators (by number).
    fn operator_map(ch: usize) -> [usize; 4];
    /// A register write; Some(channel, operator mask) for a key on/off.
    fn write(&mut self, index: u16, data: u8) -> Option<(usize, u32)>;
    fn clock_noise_and_lfo(&mut self) -> i32;
    fn lfo_am_offset(&self, choffs: u32) -> u32;
    fn noise_state(&self) -> u32 {
        0
    }
    fn noise_enable(&self) -> bool {
        false
    }
    fn cache_operator_data(&self, choffs: u32, opoffs: u32, cache: &mut OpCache);
    fn compute_phase_step(&self, choffs: u32, opoffs: u32, cache: &OpCache, lfo_raw_pm: i32) -> u32;
    fn ch_feedback(&self, choffs: u32) -> u32;
    fn ch_algorithm(&self, choffs: u32) -> u32;
    fn ch_output_any(&self, choffs: u32) -> u32;
    fn ch_output_0(&self, choffs: u32) -> u32;
    fn ch_output_1(&self, choffs: u32) -> u32;
    fn op_lfo_am_enable(&self, opoffs: u32) -> u32;
    fn op_ssg_eg_enable(&self, _opoffs: u32) -> u32 {
        0
    }
    fn op_ssg_eg_mode(&self, _opoffs: u32) -> u32 {
        0
    }
    fn waveform(&self) -> &[u16; WAVEFORM_LENGTH];
}

fn sine_waveform() -> Box<[u16; WAVEFORM_LENGTH]> {
    let mut w = Box::new([0u16; WAVEFORM_LENGTH]);
    for (i, x) in w.iter_mut().enumerate() {
        *x = abs_sin_attenuation(i as u32) as u16 | ((bitfield(i as u32, 9, 1) as u16) << 15);
    }
    w
}

#[derive(Clone, Copy)]
struct Operator {
    choffs: u32,
    opoffs: u32,
    phase: u32,
    env_attenuation: u16,
    env_state: usize,
    ssg_inverted: bool,
    key_state: u32,
    keyon_live: u32,
    /// A key off and on again between two samples (the real chip, its
    /// writes taking time, sees both; ymfm, taking them at once, neither).
    rekey: bool,
    cache: OpCache,
}

impl Operator {
    fn new(opoffs: u32) -> Operator {
        Operator {
            choffs: 0,
            opoffs,
            phase: 0,
            env_attenuation: 0x3ff,
            env_state: EG_RELEASE,
            ssg_inverted: false,
            key_state: 0,
            keyon_live: 0,
            rekey: false,
            cache: OpCache::default(),
        }
    }

    fn prepare<R: Regs>(&mut self, regs: &R) -> bool {
        regs.cache_operator_data(self.choffs, self.opoffs, &mut self.cache);
        if std::mem::take(&mut self.rekey) && self.keyon_live != 0 {
            self.clock_keystate(regs, 0);
        }
        self.clock_keystate(regs, u32::from(self.keyon_live != 0));
        // (KEYON_CSM, bit 2, cleared: CSM isn't used here.)
        self.keyon_live &= !4;
        self.env_state != EG_RELEASE || self.env_attenuation < EG_QUIET
    }

    fn clock<R: Regs>(&mut self, regs: &R, env_counter: u32, lfo_raw_pm: i32) {
        if regs.op_ssg_eg_enable(self.opoffs) != 0 {
            self.clock_ssg_eg_state(regs);
        } else {
            self.ssg_inverted = false;
        }
        if bitfield(env_counter, 0, 2) == 0 {
            self.clock_envelope(regs, env_counter >> 2);
        }
        let mut step = self.cache.phase_step;
        if step == PHASE_STEP_DYNAMIC {
            step = regs.compute_phase_step(self.choffs, self.opoffs, &self.cache, lfo_raw_pm);
        }
        self.phase = self.phase.wrapping_add(step);
    }

    fn phase(&self) -> u32 {
        self.phase >> 10
    }

    fn compute_volume<R: Regs>(&self, regs: &R, phase: u32, am_offset: u32) -> i32 {
        if self.env_attenuation > EG_QUIET {
            return 0;
        }
        let sin_att = u32::from(regs.waveform()[(phase as usize) & (WAVEFORM_LENGTH - 1)]);
        let env_att = self.envelope_attenuation(regs, am_offset) << 2;
        let result = attenuation_to_volume((sin_att & 0x7fff) + env_att) as i32;
        if bitfield(sin_att, 15, 1) != 0 {
            -result
        } else {
            result
        }
    }

    fn compute_noise_volume<R: Regs>(&self, regs: &R, am_offset: u32) -> i32 {
        let result = ((self.envelope_attenuation(regs, am_offset) ^ 0x3ff) << 1) as i32;
        if bitfield(regs.noise_state(), 0, 1) != 0 {
            -result
        } else {
            result
        }
    }

    fn keyonoff(&mut self, on: u32, rekey: bool) {
        if rekey && on & 1 != 0 && self.keyon_live & 1 == 0 && self.key_state != 0 {
            self.rekey = true;
        }
        self.keyon_live = (self.keyon_live & !1) | (on & 1);
    }

    fn start_attack<R: Regs>(&mut self, regs: &R, is_restart: bool) {
        if self.env_state == EG_ATTACK {
            return;
        }
        self.env_state = EG_ATTACK;
        if R::EG_HAS_SSG && !is_restart {
            self.ssg_inverted = (regs.op_ssg_eg_enable(self.opoffs) & bitfield(regs.op_ssg_eg_mode(self.opoffs), 2, 1)) != 0;
        }
        if !is_restart {
            self.phase = 0;
        }
        if self.cache.eg_rate[EG_ATTACK] >= 62 {
            self.env_attenuation = 0;
        }
    }

    fn start_release(&mut self) {
        if self.env_state >= EG_RELEASE {
            return;
        }
        self.env_state = EG_RELEASE;
        if self.ssg_inverted {
            self.env_attenuation = (0x200u16.wrapping_sub(self.env_attenuation)) & 0x3ff;
            self.ssg_inverted = false;
        }
    }

    fn clock_keystate<R: Regs>(&mut self, regs: &R, keystate: u32) {
        if keystate ^ self.key_state != 0 {
            self.key_state = keystate;
            if keystate != 0 {
                self.start_attack(regs, false);
            } else {
                self.start_release();
            }
        }
    }

    fn clock_ssg_eg_state<R: Regs>(&mut self, regs: &R) {
        if self.env_attenuation & 0x200 == 0 {
            return;
        }
        let mode = regs.op_ssg_eg_mode(self.opoffs);
        if mode & 1 != 0 {
            self.ssg_inverted = (bitfield(mode, 2, 1) ^ bitfield(mode, 1, 1)) != 0;
            if self.env_state != EG_ATTACK {
                self.env_attenuation = if self.ssg_inverted { 0x200 } else { 0x3ff };
            }
        } else {
            self.ssg_inverted ^= bitfield(mode, 1, 1) != 0;
            if self.env_state == EG_DECAY || self.env_state == EG_SUSTAIN {
                self.start_attack(regs, true);
            }
            if bitfield(mode, 1, 1) == 0 {
                self.phase = 0;
            }
        }
        if self.env_state == EG_RELEASE {
            self.env_attenuation = 0x3ff;
        }
    }

    fn clock_envelope<R: Regs>(&mut self, regs: &R, env_counter: u32) {
        if self.env_state == EG_ATTACK && self.env_attenuation == 0 {
            self.env_state = EG_DECAY;
        }
        if self.env_state == EG_DECAY && u32::from(self.env_attenuation) >= self.cache.eg_sustain {
            self.env_state = EG_SUSTAIN;
        }
        let rate = u32::from(self.cache.eg_rate[self.env_state]);
        let rate_shift = rate >> 2;
        let counter = env_counter << rate_shift;
        if bitfield(counter, 0, 11) != 0 {
            return;
        }
        let relevant = bitfield(counter, if rate_shift <= 11 { 11 } else { rate_shift }, 3);
        let increment = attenuation_increment(rate, relevant);
        if self.env_state == EG_ATTACK {
            if rate < 62 {
                // ~attenuation is negative (C's int), so this comes down.
                let a = i32::from(self.env_attenuation);
                self.env_attenuation = (a + (((!a) * increment as i32) >> 4)) as u16;
            }
        } else {
            let mut a = u32::from(self.env_attenuation);
            if regs.op_ssg_eg_enable(self.opoffs) == 0 {
                a += increment;
            } else if a < 0x200 {
                a += 4 * increment;
            }
            if a >= 0x400 {
                a = 0x3ff;
            }
            self.env_attenuation = a as u16;
        }
    }

    fn envelope_attenuation<R: Regs>(&self, regs: &R, am_offset: u32) -> u32 {
        let mut result = u32::from(self.env_attenuation);
        if R::EG_HAS_SSG && self.ssg_inverted {
            result = (0x200u32.wrapping_sub(result)) & 0x3ff;
        }
        if regs.op_lfo_am_enable(self.opoffs) != 0 {
            result += am_offset;
        }
        result += self.cache.total_level;
        result.min(0x3ff)
    }
}

#[derive(Clone, Copy)]
struct Channel {
    choffs: u32,
    feedback: [i16; 2],
    feedback_in: i16,
    ops: [usize; 4],
}

/// The FM engine: a family's registers, its operators and channels.
pub struct Engine<R: Regs> {
    pub regs: R,
    /// Whether a key turned off and on between two samples sounds again
    /// (true; false plays as ymfm does, for comparing with it).
    pub rekey: bool,
    ops: Vec<Operator>,
    chans: Vec<Channel>,
    env_counter: u32,
    active: u32,
    modified: u32,
    prepare_count: u32,
}

impl<R: Regs> Engine<R> {
    /// Whether a channel's operator (0 to 3, in algorithm order: M1, C1,
    /// M2, C2) is still sounding: not released down to silence.
    pub fn operator_on(&self, c: usize, i: usize) -> bool {
        let o = &self.ops[self.chans[c].ops[i]];
        o.env_state != EG_RELEASE || o.env_attenuation < EG_QUIET
    }

    pub fn new(regs: R) -> Engine<R> {
        let chans: Vec<Channel> = (0..R::CHANNELS)
            .map(|c| Channel {
                choffs: R::channel_offset(c),
                feedback: [0; 2],
                feedback_in: 0,
                ops: R::operator_map(c),
            })
            .collect();
        let mut ops: Vec<Operator> = (0..R::CHANNELS * 4).map(|o| Operator::new(R::operator_offset(o))).collect();
        for ch in &chans {
            for &o in &ch.ops {
                ops[o].choffs = ch.choffs;
            }
        }
        let all = (1u32 << R::CHANNELS) - 1;
        Engine {
            regs,
            rekey: true,
            ops,
            chans,
            env_counter: 0,
            active: all,
            modified: all,
            prepare_count: 0,
        }
    }

    pub fn write(&mut self, index: u16, data: u8) {
        self.modified = (1 << R::CHANNELS) - 1;
        if let Some((ch, mask)) = self.regs.write(index, data) {
            if ch < R::CHANNELS {
                for k in 0..4 {
                    let o = self.chans[ch].ops[k];
                    self.ops[o].keyonoff((mask >> k) & 1, self.rekey);
                }
            }
        }
    }

    /// One sample's clock of every channel.
    pub fn clock(&mut self) {
        if self.modified != 0 || self.prepare_count >= 4096 {
            self.active = 0;
            for c in 0..R::CHANNELS {
                let mut any = false;
                for k in 0..4 {
                    let o = self.chans[c].ops[k];
                    any |= self.ops[o].prepare(&self.regs);
                }
                if any {
                    self.active |= 1 << c;
                }
            }
            self.modified = 0;
            self.prepare_count = 0;
        } else {
            self.prepare_count += 1;
        }
        // EG_CLOCK_DIVIDER is 3 for both families.
        self.env_counter = self.env_counter.wrapping_add(1);
        if bitfield(self.env_counter, 0, 2) == 3 {
            self.env_counter = self.env_counter.wrapping_add(1);
        }
        let pm = self.regs.clock_noise_and_lfo();
        for c in 0..R::CHANNELS {
            let ch = &mut self.chans[c];
            ch.feedback[0] = ch.feedback[1];
            ch.feedback[1] = ch.feedback_in;
            for k in 0..4 {
                let o = ch.ops[k];
                self.ops[o].clock(&self.regs, self.env_counter, pm);
            }
        }
    }

    /// Channel `c`'s output, left and right, after `rshift` and clipped to `clipmax`.
    pub fn output(&mut self, c: usize, rshift: u32, clipmax: i32) -> [i32; 2] {
        if self.active & (1 << c) == 0 {
            return [0, 0];
        }
        let regs = &self.regs;
        let ch = self.chans[c];
        let op = |k: usize| &self.ops[ch.ops[k]];
        let am = regs.lfo_am_offset(ch.choffs);
        let fb = regs.ch_feedback(ch.choffs);
        let opmod = if fb != 0 { (i32::from(ch.feedback[0]) + i32::from(ch.feedback[1])) >> (10 - fb) } else { 0 };
        let op1 = op(0).compute_volume(regs, (op(0).phase() as i32 + opmod) as u32, am);
        self.chans[c].feedback_in = op1 as i16;
        if regs.ch_output_any(ch.choffs) == 0 {
            return [0, 0];
        }
        const fn alg(op2in: u16, op3in: u16, op4in: u16, o1: u16, o2: u16, o3: u16) -> u16 {
            op2in | (op3in << 1) | (op4in << 4) | (o1 << 7) | (o2 << 8) | (o3 << 9)
        }
        const ALGS: [u16; 8] = [
            alg(1, 2, 3, 0, 0, 0),
            alg(0, 5, 3, 0, 0, 0),
            alg(0, 2, 6, 0, 0, 0),
            alg(1, 0, 7, 0, 0, 0),
            alg(1, 0, 3, 0, 1, 0),
            alg(1, 1, 1, 0, 1, 1),
            alg(1, 0, 0, 0, 1, 1),
            alg(0, 0, 0, 1, 1, 1),
        ];
        let a = u32::from(ALGS[regs.ch_algorithm(ch.choffs) as usize & 7]);
        let mut out = [0i16; 8];
        out[1] = op1 as i16;
        let m = i32::from(out[bitfield(a, 0, 1) as usize]) >> 1;
        out[2] = op(1).compute_volume(regs, (op(1).phase() as i32 + m) as u32, am) as i16;
        out[5] = out[1].wrapping_add(out[2]);
        let m = i32::from(out[bitfield(a, 1, 3) as usize]) >> 1;
        out[3] = op(2).compute_volume(regs, (op(2).phase() as i32 + m) as u32, am) as i16;
        out[6] = out[1].wrapping_add(out[3]);
        out[7] = out[2].wrapping_add(out[3]);
        let mut result = if regs.noise_enable() && ch.choffs == 7 {
            op(3).compute_noise_volume(regs, am)
        } else {
            let m = i32::from(out[bitfield(a, 4, 3) as usize]) >> 1;
            op(3).compute_volume(regs, (op(3).phase() as i32 + m) as u32, am)
        };
        result >>= rshift;
        let clipmin = -clipmax - 1;
        if bitfield(a, 7, 1) != 0 {
            result = (result + (i32::from(out[1]) >> rshift)).clamp(clipmin, clipmax);
        }
        if bitfield(a, 8, 1) != 0 {
            result = (result + (i32::from(out[2]) >> rshift)).clamp(clipmin, clipmax);
        }
        if bitfield(a, 9, 1) != 0 {
            result = (result + (i32::from(out[3]) >> rshift)).clamp(clipmin, clipmax);
        }
        [if regs.ch_output_0(ch.choffs) != 0 { result } else { 0 }, if regs.ch_output_1(ch.choffs) != 0 { result } else { 0 }]
    }
}

/// The OPN family's registers with the OPNA's extras (two halves of six
/// channels, stereo, the LFO), as the YM2612 and YM2610 have them.
pub struct OpnaRegs {
    lfo_counter: u32,
    lfo_am: u8,
    data: Vec<u8>,
    wave: Box<[u16; WAVEFORM_LENGTH]>,
}

impl Default for OpnaRegs {
    fn default() -> OpnaRegs {
        let mut data = vec![0u8; 0x200];
        for i in [0xb4, 0xb5, 0xb6, 0x1b4, 0x1b5, 0x1b6] {
            data[i] = 0xc0;
        }
        OpnaRegs {
            lfo_counter: 0,
            lfo_am: 0,
            data,
            wave: sine_waveform(),
        }
    }
}

impl OpnaRegs {
    fn byte(&self, offset: u32, start: u32, count: u32, extra: u32) -> u32 {
        bitfield(u32::from(self.data[(offset + extra) as usize]), start, count)
    }
    #[allow(clippy::too_many_arguments)]
    fn word(&self, o1: u32, s1: u32, c1: u32, o2: u32, s2: u32, c2: u32, extra: u32) -> u32 {
        (self.byte(o1, s1, c1, extra) << c2) | self.byte(o2, s2, c2, extra)
    }
    pub fn read(&self, index: u16) -> u8 {
        self.data[usize::from(index) & 0x1ff]
    }
    fn lfo_enable(&self) -> u32 {
        self.byte(0x22, 3, 1, 0)
    }
    fn lfo_rate(&self) -> u32 {
        self.byte(0x22, 0, 3, 0)
    }
    fn multi_freq(&self) -> bool {
        self.byte(0x27, 6, 2, 0) != 0
    }
    fn multi_block_freq(&self, num: u32) -> u32 {
        self.word(0xac, 0, 6, 0xa8, 0, 8, num)
    }
    fn ch_block_freq(&self, choffs: u32) -> u32 {
        self.word(0xa4, 0, 6, 0xa0, 0, 8, choffs)
    }
    fn ch_lfo_am_sens(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 4, 2, choffs)
    }
    fn ch_lfo_pm_sens(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 0, 3, choffs)
    }
}

impl Regs for OpnaRegs {
    const CHANNELS: usize = 6;
    const EG_HAS_SSG: bool = true;

    fn channel_offset(ch: usize) -> u32 {
        (ch % 3) as u32 + 0x100 * (ch / 3) as u32
    }
    fn operator_offset(op: usize) -> u32 {
        ((op % 12) + (op % 12) / 3 + 0x100 * (op / 12)) as u32
    }
    fn operator_map(ch: usize) -> [usize; 4] {
        const MAP: [[usize; 4]; 6] = [[0, 6, 3, 9], [1, 7, 4, 10], [2, 8, 5, 11], [12, 18, 15, 21], [13, 19, 16, 22], [14, 20, 17, 23]];
        MAP[ch]
    }
    fn write(&mut self, index: u16, data: u8) -> Option<(usize, u32)> {
        let index = usize::from(index) & 0x1ff;
        if index & 0xf0 == 0xa0 {
            if index & 3 == 3 {
                return None;
            }
            let latch = 0xb8 | ((index >> 3) & 1);
            if index & 4 != 0 {
                self.data[latch] = data & 0x3f;
            } else {
                self.data[index] = data;
                self.data[index | 4] = self.data[latch];
            }
            return None;
        } else if index & 0xf8 == 0xb8 {
            return None;
        }
        self.data[index] = data;
        if index == 0x28 {
            let mut ch = u32::from(data & 3);
            if ch == 3 {
                return None;
            }
            ch += u32::from((data >> 2) & 1) * 3;
            return Some((ch as usize, u32::from(data >> 4)));
        }
        None
    }
    fn clock_noise_and_lfo(&mut self) -> i32 {
        if self.lfo_enable() == 0 {
            self.lfo_counter = 0;
            self.lfo_am = 0x3f;
            return 0;
        }
        const MAX: [u8; 8] = [109, 78, 72, 68, 63, 45, 9, 6];
        let sub = self.lfo_counter & 0xff;
        self.lfo_counter += 1;
        if sub >= u32::from(MAX[self.lfo_rate() as usize]) {
            self.lfo_counter += 0x101 - sub;
        }
        self.lfo_am = bitfield(self.lfo_counter, 8, 6) as u8;
        if bitfield(self.lfo_counter, 14, 1) == 0 {
            self.lfo_am ^= 0x3f;
        }
        let mut pm = bitfield(self.lfo_counter, 10, 3) as i32;
        if bitfield(self.lfo_counter, 13, 1) != 0 {
            pm ^= 7;
        }
        if bitfield(self.lfo_counter, 14, 1) != 0 {
            -pm
        } else {
            pm
        }
    }
    fn lfo_am_offset(&self, choffs: u32) -> u32 {
        let am_shift = (1 << (self.ch_lfo_am_sens(choffs) ^ 3)) - 1;
        (u32::from(self.lfo_am) << 1) >> am_shift
    }
    fn cache_operator_data(&self, choffs: u32, opoffs: u32, cache: &mut OpCache) {
        let mut bf = self.ch_block_freq(choffs);
        if self.multi_freq() && choffs == 2 {
            match opoffs {
                2 => bf = self.multi_block_freq(1),
                10 => bf = self.multi_block_freq(2),
                6 => bf = self.multi_block_freq(0),
                _ => {}
            }
        }
        cache.block_freq = bf;
        let mut keycode = bitfield(bf, 10, 4) << 1;
        keycode |= bitfield(0xfe80, bitfield(bf, 7, 4), 1);
        cache.detune = detune_adjustment(self.byte(0x30, 4, 3, opoffs), keycode);
        cache.multiple = self.byte(0x30, 0, 4, opoffs) * 2;
        if cache.multiple == 0 {
            cache.multiple = 1;
        }
        cache.phase_step = if self.lfo_enable() == 0 || self.ch_lfo_pm_sens(choffs) == 0 {
            self.compute_phase_step(choffs, opoffs, cache, 0)
        } else {
            PHASE_STEP_DYNAMIC
        };
        cache.total_level = self.byte(0x40, 0, 7, opoffs) << 3;
        let mut sl = self.byte(0x80, 4, 4, opoffs);
        sl |= (sl + 1) & 0x10;
        cache.eg_sustain = sl << 5;
        let ksr = keycode >> (self.byte(0x50, 6, 2, opoffs) ^ 3);
        cache.eg_rate[EG_ATTACK] = effective_rate(self.byte(0x50, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_DECAY] = effective_rate(self.byte(0x60, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_SUSTAIN] = effective_rate(self.byte(0x70, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_RELEASE] = effective_rate(self.byte(0x80, 0, 4, opoffs) * 4 + 2, ksr);
    }
    fn compute_phase_step(&self, choffs: u32, _opoffs: u32, cache: &OpCache, lfo_raw_pm: i32) -> u32 {
        let mut fnum = bitfield(cache.block_freq, 0, 11) << 1;
        let pms = self.ch_lfo_pm_sens(choffs);
        if pms != 0 {
            fnum = (fnum as i32 + opn_lfo_pm_phase_adjustment(bitfield(cache.block_freq, 4, 7), pms, lfo_raw_pm)) as u32;
            fnum &= 0xfff;
        }
        let block = bitfield(cache.block_freq, 11, 3);
        let mut step = (fnum << block) >> 2;
        step = (step as i32 + cache.detune) as u32;
        step &= 0x1ffff;
        (step * cache.multiple) >> 1
    }
    fn ch_feedback(&self, choffs: u32) -> u32 {
        self.byte(0xb0, 3, 3, choffs)
    }
    fn ch_algorithm(&self, choffs: u32) -> u32 {
        self.byte(0xb0, 0, 3, choffs)
    }
    fn ch_output_any(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 6, 2, choffs)
    }
    fn ch_output_0(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 7, 1, choffs)
    }
    fn ch_output_1(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 6, 1, choffs)
    }
    fn op_lfo_am_enable(&self, opoffs: u32) -> u32 {
        self.byte(0x60, 7, 1, opoffs)
    }
    fn op_ssg_eg_enable(&self, opoffs: u32) -> u32 {
        self.byte(0x90, 3, 1, opoffs)
    }
    fn op_ssg_eg_mode(&self, opoffs: u32) -> u32 {
        self.byte(0x90, 0, 3, opoffs)
    }
    fn waveform(&self) -> &[u16; WAVEFORM_LENGTH] {
        &self.wave
    }
}

/// The OPM's registers (the YM2151).
pub struct OpmRegs {
    lfo_counter: u32,
    noise_lfsr: u32,
    noise_counter: u8,
    noise_state: u8,
    lfo_am: u8,
    data: Vec<u8>,
    lfo_wave: [[i16; 256]; 4],
    wave: Box<[u16; WAVEFORM_LENGTH]>,
}

impl Default for OpmRegs {
    fn default() -> OpmRegs {
        let mut lfo_wave = [[0i16; 256]; 4];
        for i in 0..256u32 {
            let (am, pm) = ((i ^ 0xff) as u8, i as u8);
            lfo_wave[0][i as usize] = (u16::from(am) | (u16::from(pm) << 8)) as i16;
            let am = if bitfield(i, 7, 1) != 0 { 0u8 } else { 0xff };
            let pm = am ^ 0x80;
            lfo_wave[1][i as usize] = (u16::from(am) | (u16::from(pm) << 8)) as i16;
            let am = if bitfield(i, 7, 1) != 0 { (i << 1) as u8 } else { ((i ^ 0xff) << 1) as u8 };
            let pm = if bitfield(i, 6, 1) != 0 { am } else { !am };
            lfo_wave[2][i as usize] = (u16::from(am) | (u16::from(pm) << 8)) as i16;
        }
        let mut data = vec![0u8; 0x100];
        for i in 0x20..0x28 {
            data[i] = 0xc0;
        }
        OpmRegs {
            lfo_counter: 0,
            noise_lfsr: 1,
            noise_counter: 0,
            noise_state: 0,
            lfo_am: 0,
            data,
            lfo_wave,
            wave: sine_waveform(),
        }
    }
}

impl OpmRegs {
    fn byte(&self, offset: u32, start: u32, count: u32, extra: u32) -> u32 {
        bitfield(u32::from(self.data[((offset + extra) & 0xff) as usize]), start, count)
    }
    pub fn read(&self, index: u8) -> u8 {
        self.data[usize::from(index)]
    }
    fn noise_frequency(&self) -> u32 {
        self.byte(0x0f, 0, 5, 0) ^ 0x1f
    }
    fn lfo_rate(&self) -> u32 {
        self.byte(0x18, 0, 8, 0)
    }
    fn lfo_am_depth(&self) -> u32 {
        self.byte(0x19, 0, 7, 0)
    }
    fn lfo_pm_depth(&self) -> u32 {
        self.byte(0x1a, 0, 7, 0)
    }
    fn lfo_waveform(&self) -> usize {
        self.byte(0x1b, 0, 2, 0) as usize
    }
    fn ch_block_freq(&self, choffs: u32) -> u32 {
        (self.byte(0x28, 0, 7, choffs) << 6) | self.byte(0x30, 2, 6, choffs)
    }
    fn ch_lfo_pm_sens(&self, choffs: u32) -> u32 {
        self.byte(0x38, 4, 3, choffs)
    }
    fn ch_lfo_am_sens(&self, choffs: u32) -> u32 {
        self.byte(0x38, 0, 2, choffs)
    }
}

impl Regs for OpmRegs {
    const CHANNELS: usize = 8;
    const EG_HAS_SSG: bool = false;

    fn channel_offset(ch: usize) -> u32 {
        ch as u32
    }
    fn operator_offset(op: usize) -> u32 {
        op as u32
    }
    fn operator_map(ch: usize) -> [usize; 4] {
        [ch, ch + 16, ch + 8, ch + 24]
    }
    fn write(&mut self, index: u16, data: u8) -> Option<(usize, u32)> {
        let index = usize::from(index) & 0xff;
        if index == 0x19 {
            self.data[index + usize::from(data >> 7)] = data;
        } else if index != 0x1a {
            self.data[index] = data;
        }
        if index == 0x08 {
            return Some((usize::from(data & 7), u32::from((data >> 3) & 15)));
        }
        None
    }
    fn clock_noise_and_lfo(&mut self) -> i32 {
        let freq = self.noise_frequency();
        for _ in 0..2 {
            self.noise_lfsr <<= 1;
            self.noise_lfsr |= bitfield(self.noise_lfsr, 17, 1) ^ bitfield(self.noise_lfsr, 14, 1) ^ 1;
            let c = u32::from(self.noise_counter);
            self.noise_counter = self.noise_counter.wrapping_add(1);
            if c >= freq {
                self.noise_counter = 0;
                self.noise_state = bitfield(self.noise_lfsr, 17, 1) as u8;
            }
        }
        let rate = self.lfo_rate();
        self.lfo_counter = self.lfo_counter.wrapping_add((0x10 | bitfield(rate, 0, 4)) << bitfield(rate, 4, 4));
        if self.byte(0x01, 1, 1, 0) != 0 {
            self.lfo_counter = 0;
        }
        let lfo = bitfield(self.lfo_counter, 22, 8) as usize;
        let noise = bitfield(self.noise_lfsr, 17, 8);
        self.lfo_wave[3][(lfo + 1) & 0xff] = (noise | (noise << 8)) as u16 as i16;
        let ampm = i32::from(self.lfo_wave[self.lfo_waveform()][lfo]);
        self.lfo_am = (((ampm & 0xff) as u32 * self.lfo_am_depth()) >> 7) as u8;
        ((ampm >> 8) * self.lfo_pm_depth() as i32) >> 7
    }
    fn lfo_am_offset(&self, choffs: u32) -> u32 {
        let s = self.ch_lfo_am_sens(choffs);
        if s == 0 {
            0
        } else {
            u32::from(self.lfo_am) << (s - 1)
        }
    }
    fn noise_state(&self) -> u32 {
        u32::from(self.noise_state)
    }
    fn noise_enable(&self) -> bool {
        self.byte(0x0f, 7, 1, 0) != 0
    }
    fn cache_operator_data(&self, choffs: u32, opoffs: u32, cache: &mut OpCache) {
        let bf = self.ch_block_freq(choffs);
        cache.block_freq = bf;
        let keycode = bitfield(bf, 8, 5);
        cache.detune = detune_adjustment(self.byte(0x40, 4, 3, opoffs), keycode);
        cache.multiple = self.byte(0x40, 0, 4, opoffs) * 2;
        if cache.multiple == 0 {
            cache.multiple = 1;
        }
        cache.phase_step = if self.lfo_pm_depth() == 0 || self.ch_lfo_pm_sens(choffs) == 0 {
            self.compute_phase_step(choffs, opoffs, cache, 0)
        } else {
            PHASE_STEP_DYNAMIC
        };
        cache.total_level = self.byte(0x60, 0, 7, opoffs) << 3;
        let mut sl = self.byte(0xe0, 4, 4, opoffs);
        sl |= (sl + 1) & 0x10;
        cache.eg_sustain = sl << 5;
        let ksr = keycode >> (self.byte(0x80, 6, 2, opoffs) ^ 3);
        cache.eg_rate[EG_ATTACK] = effective_rate(self.byte(0x80, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_DECAY] = effective_rate(self.byte(0xa0, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_SUSTAIN] = effective_rate(self.byte(0xc0, 0, 5, opoffs) * 2, ksr);
        cache.eg_rate[EG_RELEASE] = effective_rate(self.byte(0xe0, 0, 4, opoffs) * 4 + 2, ksr);
    }
    fn compute_phase_step(&self, choffs: u32, opoffs: u32, cache: &OpCache, lfo_raw_pm: i32) -> u32 {
        const DT2: [i32; 4] = [0, (600 * 64 + 50) / 100, (781 * 64 + 50) / 100, (950 * 64 + 50) / 100];
        let mut delta = DT2[self.byte(0xc0, 6, 2, opoffs) as usize];
        let pms = self.ch_lfo_pm_sens(choffs);
        if pms != 0 {
            if pms < 6 {
                delta += lfo_raw_pm >> (6 - pms);
            } else {
                delta = delta.wrapping_add(((lfo_raw_pm as u32) << (pms - 5)) as i32);
            }
        }
        let mut step = opm_key_code_to_phase_step(cache.block_freq, delta);
        step = (step as i32 + cache.detune) as u32;
        (step.wrapping_mul(cache.multiple)) >> 1
    }
    fn ch_feedback(&self, choffs: u32) -> u32 {
        self.byte(0x20, 3, 3, choffs)
    }
    fn ch_algorithm(&self, choffs: u32) -> u32 {
        self.byte(0x20, 0, 3, choffs)
    }
    fn ch_output_any(&self, choffs: u32) -> u32 {
        self.byte(0x20, 6, 2, choffs)
    }
    fn ch_output_0(&self, choffs: u32) -> u32 {
        self.byte(0x20, 6, 1, choffs)
    }
    fn ch_output_1(&self, choffs: u32) -> u32 {
        self.byte(0x20, 7, 1, choffs)
    }
    fn op_lfo_am_enable(&self, opoffs: u32) -> u32 {
        self.byte(0xa0, 7, 1, opoffs)
    }
    fn waveform(&self) -> &[u16; WAVEFORM_LENGTH] {
        &self.wave
    }
}

/// The YM2612 (OPN2): six FM channels, the last of which the DAC can
/// take over, its 9-bit output's crossover distortion modelled.
pub struct Ym2612 {
    pub fm: Engine<OpnaRegs>,
    dac_data: u16,
    dac_enable: bool,
}

impl Default for Ym2612 {
    fn default() -> Ym2612 {
        Ym2612 {
            fm: Engine::new(OpnaRegs::default()),
            dac_data: 0,
            dac_enable: false,
        }
    }
}

impl Ym2612 {
    /// The chip's sample rate at `clock`.
    pub fn rate(clock: f64) -> f64 {
        clock / 144.0
    }

    /// A write to register `reg` of half `hi` (0 or 1).
    pub fn write(&mut self, hi: bool, reg: u8, data: u8) {
        if hi {
            self.fm.write(0x100 | u16::from(reg), data);
            return;
        }
        match reg {
            0x2a => self.dac_data = (self.dac_data & !0x1fe) | (u16::from(data ^ 0x80) << 1),
            0x2b => self.dac_enable = data & 0x80 != 0,
            0x2c => self.dac_data = (self.dac_data & !1) | u16::from((data >> 3) & 1),
            _ => self.fm.write(u16::from(reg), data),
        }
    }

    pub fn dac_enabled(&self) -> bool {
        self.dac_enable
    }

    fn discontinuity(v: i32) -> i32 {
        if v < 0 {
            v - 3
        } else {
            v + 4
        }
    }

    /// One sample: each channel's left and right (channel 6 the DAC's when
    /// it's on), its crossover step applied but not yet scaled: the chip's
    /// output is their sum times 128 × 64 / (6 × 65).
    pub fn generate(&mut self, out: &mut [[i32; 2]; 6]) {
        self.fm.clock();
        let last = if self.dac_enable { 5 } else { 6 };
        for (c, o) in out.iter_mut().enumerate().take(last) {
            let v = self.fm.output(c, 5, 256);
            *o = [Ym2612::discontinuity(v[0]), Ym2612::discontinuity(v[1])];
        }
        if self.dac_enable {
            let dac = Ym2612::discontinuity(i32::from(((self.dac_data << 7) as i16) >> 7));
            let regs = &self.fm.regs;
            out[5] = [
                if regs.ch_output_0(0x102) != 0 { dac } else { Ym2612::discontinuity(0) },
                if regs.ch_output_1(0x102) != 0 { dac } else { Ym2612::discontinuity(0) },
            ];
        }
    }
}

/// The YM2151 (OPM): eight FM channels, the last with noise, through the
/// YM3012 DAC's floating-point format.
pub struct Ym2151 {
    pub fm: Engine<OpmRegs>,
}

impl Default for Ym2151 {
    fn default() -> Ym2151 {
        Ym2151 { fm: Engine::new(OpmRegs::default()) }
    }
}

/// A value as the YM3012's 10.3 floating point keeps it.
fn roundtrip_fp(v: i32) -> i32 {
    let v = v.clamp(-32768, 32767);
    let scan = v ^ (v >> 31);
    let exponent = (7 - ((scan << 17) as u32).leading_zeros() as i32).max(1) - 1;
    v & !((1 << exponent) - 1)
}

impl Ym2151 {
    pub fn rate(clock: f64) -> f64 {
        clock / 64.0
    }

    pub fn write(&mut self, reg: u8, data: u8) {
        self.fm.write(u16::from(reg), data);
    }

    /// One sample: each channel's left and right.
    pub fn generate(&mut self, out: &mut [[i32; 2]; 8]) {
        self.fm.clock();
        for (c, o) in out.iter_mut().enumerate() {
            let v = self.fm.output(c, 0, 32767);
            *o = [roundtrip_fp(v[0]), roundtrip_fp(v[1])];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a VGM's YM2612 writes (YM_VGM) and compares the mix, sample by
    /// sample, with the C++ ymfm's (YM_REF, int32 left/right pairs, from
    /// `vgm2612`).
    #[test]
    #[ignore]
    fn ym2612_against_ymfm() {
        let d = std::fs::read(std::env::var("YM_VGM").unwrap()).unwrap();
        let r = std::fs::read(std::env::var("YM_REF").unwrap()).unwrap();
        let reference: Vec<i32> = r.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let u32at = |p: usize| u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]);
        let ver = u32at(8);
        let clk = match u32at(0x2c) {
            0 => 7_670_454,
            c => c,
        };
        let mut p = if ver >= 0x150 { 0x34 + u32at(0x34) as usize } else { 0x40 };
        let rate = f64::from(clk) / 144.0;
        let mut acc = 0.0;
        let mut chip = Ym2612::default();
        chip.fm.rekey = std::env::var("YM_REKEY").is_ok();
        let mut out = Vec::new();
        let most = reference.len() / 2;
        let mut gen = |chip: &mut Ym2612, n: usize, out: &mut Vec<i32>| {
            for _ in 0..n {
                if out.len() / 2 >= most {
                    return;
                }
                let mut o = [[0; 2]; 6];
                chip.generate(&mut o);
                // YM_SOLO=n: that channel alone.
                if let Some(k) = std::env::var("YM_SOLO").ok().and_then(|v| v.parse::<usize>().ok()) {
                    for (c, x) in o.iter_mut().enumerate() {
                        if c != k {
                            *x = [0, 0];
                        }
                    }
                }
                let (l, r) = o.iter().fold((0, 0), |a, x| (a.0 + x[0], a.1 + x[1]));
                out.push(l * 128 * 64 / (6 * 65));
                out.push(r * 128 * 64 / (6 * 65));
            }
        };
        while p < d.len() && out.len() / 2 < most {
            let c = d[p];
            let mut wait = |n: f64, chip: &mut Ym2612, out: &mut Vec<i32>| {
                acc += n * rate / 44100.0;
                let k = acc as usize;
                acc -= k as f64;
                gen(chip, k, out);
            };
            match c {
                0x52 | 0x53 => {
                    chip.write(c == 0x53, d[p + 1], d[p + 2]);
                    p += 3;
                }
                0x61 => {
                    wait(f64::from(u16::from_le_bytes([d[p + 1], d[p + 2]])), &mut chip, &mut out);
                    p += 3;
                }
                0x62 => {
                    wait(735.0, &mut chip, &mut out);
                    p += 1;
                }
                0x63 => {
                    wait(882.0, &mut chip, &mut out);
                    p += 1;
                }
                0x70..=0x7f => {
                    wait(f64::from((c & 15) + 1), &mut chip, &mut out);
                    p += 1;
                }
                0x80..=0x8f => {
                    wait(f64::from(c & 15), &mut chip, &mut out);
                    p += 1;
                }
                0x66 => break,
                0x67 => p += 7 + u32at(p + 3) as usize,
                0x4f | 0x50 | 0x30..=0x3f | 0x94 => p += 2,
                0x51 | 0x54..=0x5f | 0xa0..=0xbf | 0x40..=0x4e => p += 3,
                0xc0..=0xdf => p += 4,
                0xe0..=0xff | 0x90 | 0x91 | 0x95 => p += 5,
                0x92 => p += 6,
                0x93 => p += 11,
                _ => panic!("command {c:#x}"),
            }
        }
        if let Ok(w) = std::env::var("YM_WAV") {
            // The output as a 53,267 Hz WAV, for listening or comparing.
            let mut b = Vec::new();
            let len = (out.len() * 2) as u32;
            b.extend_from_slice(b"RIFF");
            b.extend_from_slice(&(36 + len).to_le_bytes());
            b.extend_from_slice(b"WAVEfmt ");
            b.extend_from_slice(&16u32.to_le_bytes());
            b.extend_from_slice(&[1, 0, 2, 0]);
            b.extend_from_slice(&(rate as u32).to_le_bytes());
            b.extend_from_slice(&(rate as u32 * 4).to_le_bytes());
            b.extend_from_slice(&[4, 0, 16, 0]);
            b.extend_from_slice(b"data");
            b.extend_from_slice(&len.to_le_bytes());
            for v in &out {
                b.extend_from_slice(&((*v).clamp(-32768, 32767) as i16).to_le_bytes());
            }
            std::fs::write(w, b).unwrap();
            return;
        }
        let n = out.len().min(reference.len());
        let first = (0..n).find(|&i| out[i] != reference[i]);
        println!("{} samples compared, first difference at {:?}", n / 2, first.map(|i| i / 2));
        assert!(first.is_none());
    }
}
