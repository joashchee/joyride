// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// NSD.Lib compiler's command tables (nsc 1.31, S.W., BSD-2): generated from its source, the keywords as the
// Shift JIS bytes its .mml files use. Each module holds a table's command ids, in the order of its enum.

#![allow(non_upper_case_globals, dead_code)]

pub mod mf {
    pub const id_Title: u32 = 0;
    pub const id_Composer: u32 = 1;
    pub const id_Copyright: u32 = 2;
    pub const id_Maker: u32 = 3;
    pub const id_Text: u32 = 4;
    pub const id_plst: u32 = 5;
    pub const id_psfx: u32 = 6;
    pub const id_mixe: u32 = 7;
    pub const id_vrc7_chg: u32 = 8;
    pub const id_OffsetPCM: u32 = 9;
    pub const id_Code: u32 = 10;
    pub const id_External: u32 = 11;
    pub const id_Bank: u32 = 12;
    pub const id_SegmentSEQ: u32 = 13;
    pub const id_SegmentPCM: u32 = 14;
    pub const id_Label: u32 = 15;
    pub const id_Priority: u32 = 16;
    pub const id_include: u32 = 17;
    pub const id_timebase: u32 = 18;
    pub const id_OctaveReverse: u32 = 19;
    pub const id_QReverse: u32 = 20;
    pub const id_bgm_num: u32 = 21;
    pub const id_se_num: u32 = 22;
    pub const id_releaseVolume: u32 = 23;
    pub const id_repeatMode: u32 = 24;
    pub const id_TieMode: u32 = 25;
    pub const id_offset_Ei: u32 = 26;
    pub const id_offset_Ev: u32 = 27;
    pub const id_offset_En: u32 = 28;
    pub const id_offset_Em: u32 = 29;
    pub const id_QMax: u32 = 30;
    pub const id_rest: u32 = 31;
    pub const id_wait: u32 = 32;
    pub const id_DPCM: u32 = 33;
    pub const id_FDSC: u32 = 34;
    pub const id_FDSM: u32 = 35;
    pub const id_VRC7: u32 = 36;
    pub const id_N163: u32 = 37;
    pub const id_Envelop: u32 = 38;
    pub const id_Vibrato: u32 = 39;
    pub const id_Macro: u32 = 40;
    pub const id_Sub: u32 = 41;
    pub const id_BGM: u32 = 42;
    pub const id_SE: u32 = 43;
    pub const id_Patch: u32 = 44;
    pub const id_Null: u32 = 45;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"#Title", id_Title),
        (b"#title", id_Title),
        (b"\x8b\xc8\x96\xbc", id_Title),
        (b"#Composer", id_Composer),
        (b"#composer", id_Composer),
        (b"\x8d\xec\x8b\xc8\x8e\xd2", id_Composer),
        (b"\x8d\xec\x8b\xc8", id_Composer),
        (b"#Copyright", id_Copyright),
        (b"#copyright", id_Copyright),
        (b"\x8d\xec\x8e\xd2", id_Copyright),
        (b"\x92\x98\x8d\xec\x8c\xa0\x8e\xd2", id_Copyright),
        (b"#Maker", id_Maker),
        (b"#maker", id_Maker),
        (b"#Ripper", id_Maker),
        (b"#ripper", id_Maker),
        (b"\x90\xbb\x8d\xec\x8e\xd2", id_Maker),
        (b"#Text", id_Text),
        (b"#text", id_Text),
        (b"\x83e\x83L\x83X\x83g", id_Text),
        (b"#PlayList", id_plst),
        (b"#Plst", id_plst),
        (b"#plst", id_plst),
        (b"\x89\xb9\x8ay\x8f\x87\x94\xd4", id_plst),
        (b"\x8b\xc8\x8f\x87", id_plst),
        (b"#PlaySfx", id_psfx),
        (b"#Psfx", id_psfx),
        (b"#psfx", id_psfx),
        (b"\x8c\xf8\x89\xca\x89\xb9\x8f\x87\x94\xd4", id_psfx),
        (b"#Mixe", id_mixe),
        (b"#mixe", id_mixe),
        (b"#Mix", id_mixe),
        (b"#mix", id_mixe),
        (b"#VRC7", id_vrc7_chg),
        (b"#OffsetPCM", id_OffsetPCM),
        (b"#offsetPCM", id_OffsetPCM),
        (b"#Code", id_Code),
        (b"#code", id_Code),
        (b"\x83R\x81[\x83h", id_Code),
        (b"#External", id_External),
        (b"#external", id_External),
        (b"#Bank", id_Bank),
        (b"#bank", id_Bank),
        (b"\x83o\x83\x93\x83N", id_Bank),
        (b"#SegmentPCM", id_SegmentPCM),
        (b"#segmentPCM", id_SegmentPCM),
        (b"#SegmentSEQ", id_SegmentSEQ),
        (b"#segmentSEQ", id_SegmentSEQ),
        (b"#Segment", id_SegmentSEQ),
        (b"#segment", id_SegmentSEQ),
        (b"#Label", id_Label),
        (b"#label", id_Label),
        (b"#Include", id_include),
        (b"#include", id_include),
        (b"\x93\xc7\x82\xde", id_include),
        (b"#Timebase", id_timebase),
        (b"#timebase", id_timebase),
        (b"#OctaveReverse", id_OctaveReverse),
        (b"#octaveReverse", id_OctaveReverse),
        (b"#QReverse", id_QReverse),
        (b"#qReverse", id_QReverse),
        (b"#BGM", id_bgm_num),
        (b"#bgm", id_bgm_num),
        (b"\x89\xb9\x8ay\x90\x94", id_bgm_num),
        (b"#SE", id_se_num),
        (b"#se", id_se_num),
        (b"\x8c\xf8\x89\xca\x89\xb9\x90\x94", id_se_num),
        (b"#releaseVolume", id_releaseVolume),
        (b"#ReleaseVolume", id_releaseVolume),
        (b"#repeatMode", id_repeatMode),
        (b"#RepeatMode", id_repeatMode),
        (b"#TieMode", id_TieMode),
        (b"#tieMode", id_TieMode),
        (b"#OffsetE@", id_offset_Ei),
        (b"#offsetE@", id_offset_Ei),
        (b"#OffsetEv", id_offset_Ev),
        (b"#offsetEv", id_offset_Ev),
        (b"#OffsetEm", id_offset_Em),
        (b"#offsetEm", id_offset_Em),
        (b"#OffsetEn", id_offset_En),
        (b"#offsetEn", id_offset_En),
        (b"#Priority", id_Priority),
        (b"#priority", id_Priority),
        (b"#QMax", id_QMax),
        (b"#Qmax", id_QMax),
        (b"#Rest", id_rest),
        (b"#rest", id_rest),
        (b"#Wait", id_wait),
        (b"#wait", id_wait),
        (b"DPCM", id_DPCM),
        (b"FDSC", id_FDSC),
        (b"FDSM", id_FDSM),
        (b"VRC7", id_VRC7),
        (b"N163", id_N163),
        (b"Envelope", id_Envelop),
        (b"envelope", id_Envelop),
        (b"Envelop", id_Envelop),
        (b"envelop", id_Envelop),
        (b"\x83G\x83\x93\x83x\x83\x8d\x81[\x83v", id_Envelop),
        (b"\x83r\x83u\x83\x89\x81[\x83g", id_Vibrato),
        (b"Sub", id_Sub),
        (b"sub", id_Sub),
        (b"BGM", id_BGM),
        (b"bgm", id_BGM),
        (b"\x89\xb9\x8ay", id_BGM),
        (b"SE", id_SE),
        (b"se", id_SE),
        (b"\x8c\xf8\x89\xca\x89\xb9", id_SE),
        (b"Patch", id_Patch),
        (b"D", id_DPCM),
        (b"FC", id_FDSC),
        (b"FM", id_FDSM),
        (b"O", id_VRC7),
        (b"V", id_VRC7),
        (b"N", id_N163),
        (b"E", id_Envelop),
        (b"S", id_Sub),
        (b"P", id_Patch),
        (b"$", id_Macro),
        (b"@DPCM", id_DPCM),
        (b"@FM", id_FDSC),
        (b"@MW", id_FDSM),
        (b"@OP", id_VRC7),
        (b"@N", id_N163),
        (b"@E", id_Envelop),
        (b"\x81@", id_Null),
    ];
}

pub mod ts {
    pub const mml_label: u32 = 0;
    pub const mml_author: u32 = 1;
    pub const mml_time: u32 = 2;
    pub const mml_fade: u32 = 3;
    pub const mml_Track: u32 = 4;
    pub const mml_Scale: u32 = 5;
    pub const mml_Major: u32 = 6;
    pub const mml_Minor: u32 = 7;
    pub const mml_HMinor: u32 = 8;
    pub const mml_MMinor: u32 = 9;
    pub const mml_KeySignature: u32 = 10;
    pub const mml_Macro: u32 = 11;
    pub const mml_MacroSet: u32 = 12;
    pub const mml_CallSE: u32 = 13;
    pub const mml_Subroutine: u32 = 14;
    pub const mml_Loop: u32 = 15;
    pub const mml_Repeat_A_Start: u32 = 16;
    pub const mml_Repeat_A_Branch: u32 = 17;
    pub const mml_Repeat_A_End: u32 = 18;
    pub const mml_Repeat_B_Start: u32 = 19;
    pub const mml_Repeat_B_Branch: u32 = 20;
    pub const mml_Repeat_B_End: u32 = 21;
    pub const mml_Repeat_C_Start: u32 = 22;
    pub const mml_Repeat_C_End: u32 = 23;
    pub const mml_Tempo: u32 = 24;
    pub const mml_Tempo_Relative: u32 = 25;
    pub const mml_Tempo_Up: u32 = 26;
    pub const mml_Tempo_Down: u32 = 27;
    pub const mml_La: u32 = 28;
    pub const mml_Si: u32 = 29;
    pub const mml_Do: u32 = 30;
    pub const mml_Re: u32 = 31;
    pub const mml_Mi: u32 = 32;
    pub const mml_Fa: u32 = 33;
    pub const mml_Sol: u32 = 34;
    pub const mml_Rest: u32 = 35;
    pub const mml_Wait: u32 = 36;
    pub const mml_Tai: u32 = 37;
    pub const mml_Length: u32 = 38;
    pub const mml_Gate_Q: u32 = 39;
    pub const mml_Gate_q: u32 = 40;
    pub const mml_Gate_u: u32 = 41;
    pub const mml_Echo: u32 = 42;
    pub const mml_Echo_Off: u32 = 43;
    pub const mml_Echo_Reset: u32 = 44;
    pub const mml_Echo_C: u32 = 45;
    pub const mml_Echo_D: u32 = 46;
    pub const mml_Echo_E: u32 = 47;
    pub const mml_Echo_F: u32 = 48;
    pub const mml_Echo_G: u32 = 49;
    pub const mml_Echo_A: u32 = 50;
    pub const mml_Echo_B: u32 = 51;
    pub const mml_Echo_R: u32 = 52;
    pub const mml_Jump: u32 = 53;
    pub const mml_Jump_drv: u32 = 54;
    pub const mml_Envelop_Voice: u32 = 55;
    pub const mml_Envelop_Volume: u32 = 56;
    pub const mml_Envelop_Frequency: u32 = 57;
    pub const mml_Envelop_Note: u32 = 58;
    pub const mml_Envelop_Off_Voice: u32 = 59;
    pub const mml_Envelop_Off_Volume: u32 = 60;
    pub const mml_Envelop_Off_Frequency: u32 = 61;
    pub const mml_Envelop_Off_Note: u32 = 62;
    pub const mml_Patch: u32 = 63;
    pub const mml_Patch_Off: u32 = 64;
    pub const mml_Release_mdoe: u32 = 65;
    pub const mml_Release_Voice: u32 = 66;
    pub const mml_Release_Volume: u32 = 67;
    pub const mml_Voice: u32 = 68;
    pub const mml_FDSC: u32 = 69;
    pub const mml_FDSM: u32 = 70;
    pub const mml_FDSF: u32 = 71;
    pub const mml_FDSV: u32 = 72;
    pub const mml_VRC7: u32 = 73;
    pub const mml_N163: u32 = 74;
    pub const mml_N163_Set: u32 = 75;
    pub const mml_N163_Load: u32 = 76;
    pub const mml_N163_Channel: u32 = 77;
    pub const mml_FME7_frequency: u32 = 78;
    pub const mml_Octave: u32 = 79;
    pub const mml_Octave_Up: u32 = 80;
    pub const mml_Octave_Down: u32 = 81;
    pub const mml_Octave_Up1: u32 = 82;
    pub const mml_Octave_Down1: u32 = 83;
    pub const mml_Detune_Cent: u32 = 84;
    pub const mml_Detune_Cent_Relative: u32 = 85;
    pub const mml_Detune_Register: u32 = 86;
    pub const mml_Detune_Register_Relative: u32 = 87;
    pub const mml_Transpose: u32 = 88;
    pub const mml_Transpose_Relative: u32 = 89;
    pub const mml_KeyShift: u32 = 90;
    pub const mml_KeyShift_Relative: u32 = 91;
    pub const mml_Protament: u32 = 92;
    pub const mml_Protament2: u32 = 93;
    pub const mml_Sweep: u32 = 94;
    pub const mml_Volume: u32 = 95;
    pub const mml_Volume_Up: u32 = 96;
    pub const mml_Volume_Down: u32 = 97;
    pub const mml_VRC7_Write: u32 = 98;
    pub const mml_Memory_Write: u32 = 99;
    pub const mml_Priority: u32 = 100;
    pub const mml_Bar: u32 = 101;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"\x8b\xc8\x96\xbc", mml_label),
        (b"\x83^\x83C\x83g\x83\x8b", mml_label),
        (b"#Title", mml_label),
        (b"#title", mml_label),
        (b"\x83\x89\x83x\x83\x8b", mml_label),
        (b"#Label", mml_label),
        (b"#label", mml_label),
        (b"#Tlbl", mml_label),
        (b"#tlbl", mml_label),
        (b"#Author", mml_author),
        (b"#author", mml_author),
        (b"\x8d\xec\x8b\xc8", mml_author),
        (b"\x8d\xec\x8b\xc8\x8e\xd2", mml_author),
        (b"#Composer", mml_author),
        (b"#composer", mml_author),
        (b"#Taut", mml_author),
        (b"#taut", mml_author),
        (b"\x83^\x83C\x83\x80", mml_time),
        (b"\x8e\x9e\x8a\xd4", mml_time),
        (b"#Time", mml_time),
        (b"#time", mml_time),
        (b"\x83t\x83F\x81[\x83h\x83A\x83E\x83g", mml_fade),
        (b"\x83t\x83F\x81[\x83h", mml_fade),
        (b"#FadeOut", mml_fade),
        (b"#Fade", mml_fade),
        (b"#fade", mml_fade),
        (b"TR", mml_Track),
        (b"\x83g\x83\x89\x83b\x83N", mml_Track),
        (b"Scale", mml_Scale),
        (b"scale", mml_Scale),
        (b"Mode", mml_Scale),
        (b"mode", mml_Scale),
        (b"Major", mml_Major),
        (b"major", mml_Major),
        (b"Minor", mml_Minor),
        (b"minor", mml_Minor),
        (b"HMinor", mml_HMinor),
        (b"hminor", mml_HMinor),
        (b"MMinor", mml_MMinor),
        (b"mminor", mml_MMinor),
        (b"K", mml_KeySignature),
        (b"\x92\xb2", mml_KeySignature),
        (b"SE", mml_CallSE),
        (b"S", mml_Subroutine),
        (b"$$", mml_MacroSet),
        (b"$", mml_Macro),
        (b"L", mml_Loop),
        (b"[:", mml_Repeat_C_Start),
        (b":]", mml_Repeat_C_End),
        (b"|:", mml_Repeat_B_Start),
        (b"\x5c", mml_Repeat_B_Branch),
        (b"\xc2\xa5", mml_Repeat_B_Branch),
        (b"\xef\xbc\xbc", mml_Repeat_B_Branch),
        (b"\xef\xbf\xa5", mml_Repeat_B_Branch),
        (b":|", mml_Repeat_B_End),
        (b"[", mml_Repeat_A_Start),
        (b":", mml_Repeat_A_Branch),
        (b"]", mml_Repeat_A_End),
        (b"\x81\x87", mml_Loop),
        (b"\x81b\x81F", mml_Repeat_B_Start),
        (b"\x81\x8f", mml_Repeat_B_Branch),
        (b"\x81_", mml_Repeat_B_Branch),
        (b"\x81F\x81b", mml_Repeat_B_End),
        (b"\x81y", mml_Repeat_A_Start),
        (b"\x81F", mml_Repeat_A_Branch),
        (b"\x81z", mml_Repeat_A_End),
        (b"t_", mml_Tempo_Relative),
        (b"t", mml_Tempo),
        (b"\x83e\x83\x93\x83|", mml_Tempo),
        (b"\x81\xa8", mml_Tempo_Up),
        (b"\x81\xa9", mml_Tempo_Down),
        (b"a", mml_La),
        (b"b", mml_Si),
        (b"c", mml_Do),
        (b"d", mml_Re),
        (b"e", mml_Mi),
        (b"f", mml_Fa),
        (b"g", mml_Sol),
        (b"r", mml_Rest),
        (b"w", mml_Wait),
        (b"^", mml_Tai),
        (b"\x82\xe7", mml_La),
        (b"\x82\xb5", mml_Si),
        (b"\x82\xc7", mml_Do),
        (b"\x82\xea", mml_Re),
        (b"\x82\xdd", mml_Mi),
        (b"\x82\xd3\x82\x9f", mml_Fa),
        (b"\x82\xd3", mml_Fa),
        (b"\x82\xbb", mml_Sol),
        (b"\x82\xf1", mml_Rest),
        (b"\x82\xc1", mml_Rest),
        (b"\x81[", mml_Tai),
        (b"\x83\x89", mml_La),
        (b"\x83V", mml_Si),
        (b"\x83h", mml_Do),
        (b"\x83\x8c", mml_Re),
        (b"\x83~", mml_Mi),
        (b"\x83t\x83@", mml_Fa),
        (b"\x83t", mml_Fa),
        (b"\x83\x5c", mml_Sol),
        (b"\x83\x93", mml_Rest),
        (b"\x83b", mml_Rest),
        (b"\x81|", mml_Tai),
        (b"\x83C", mml_La),
        (b"\x83\x8d", mml_Si),
        (b"\x83n", mml_Do),
        (b"\x83j", mml_Re),
        (b"\x83z", mml_Mi),
        (b"\x83w", mml_Fa),
        (b"\x83g", mml_Sol),
        (b"l", mml_Length),
        (b"\x89\xb9\x95\x84", mml_Length),
        (b"Q", mml_Gate_Q),
        (b"q", mml_Gate_q),
        (b"\x83Q\x81[\x83g", mml_Gate_q),
        (b"u", mml_Gate_u),
        (b"ECc", mml_Echo_C),
        (b"ECd", mml_Echo_D),
        (b"ECe", mml_Echo_E),
        (b"ECf", mml_Echo_F),
        (b"ECg", mml_Echo_G),
        (b"ECa", mml_Echo_A),
        (b"ECb", mml_Echo_B),
        (b"ECr", mml_Echo_R),
        (b"EC-", mml_Echo_Reset),
        (b"EC*", mml_Echo_Off),
        (b"EC", mml_Echo),
        (b"\x83G\x83R\x81[\x83I\x83t", mml_Echo_Off),
        (b"\x83G\x83R\x81[", mml_Echo),
        (b"J", mml_Jump),
        (b"j", mml_Jump_drv),
        (b"E@*", mml_Envelop_Off_Voice),
        (b"Ev*", mml_Envelop_Off_Volume),
        (b"Em*", mml_Envelop_Off_Frequency),
        (b"En*", mml_Envelop_Off_Note),
        (b"E@", mml_Envelop_Voice),
        (b"Ev", mml_Envelop_Volume),
        (b"Em", mml_Envelop_Frequency),
        (b"En", mml_Envelop_Note),
        (b"\x83G\x83\x93\x83x\x83\x8d\x81[\x83v\x83I\x83t", mml_Envelop_Off_Volume),
        (b"\x83G\x83\x93\x83x\x83\x8d\x81[\x83v", mml_Envelop_Volume),
        (b"\x83r\x83u\x83\x89\x81[\x83g\x83I\x83t", mml_Envelop_Off_Frequency),
        (b"\x83r\x83u\x83\x89\x81[\x83g", mml_Envelop_Frequency),
        (b"Rm", mml_Release_mdoe),
        (b"R@", mml_Release_Voice),
        (b"Rv", mml_Release_Volume),
        (b"@P*", mml_Patch_Off),
        (b"@P", mml_Patch),
        (b"@FC", mml_FDSC),
        (b"@FM", mml_FDSM),
        (b"@FF", mml_FDSF),
        (b"@FV", mml_FDSV),
        (b"@V", mml_VRC7),
        (b"@NS", mml_N163_Set),
        (b"@NL", mml_N163_Load),
        (b"@N", mml_N163),
        (b"@", mml_Voice),
        (b"\x89\xb9\x90F", mml_Voice),
        (b"NC", mml_N163_Channel),
        (b"F", mml_FME7_frequency),
        (b"o", mml_Octave),
        (b"\x89\xb9\x8aK", mml_Octave),
        (b">", mml_Octave_Up),
        (b"\x8f\xe3", mml_Octave_Up),
        (b"\x81\xaa", mml_Octave_Up),
        (b"<", mml_Octave_Down),
        (b"\x89\xba", mml_Octave_Down),
        (b"\x81\xab", mml_Octave_Down),
        (b"`", mml_Octave_Up1),
        (b"\x81e", mml_Octave_Up1),
        (b"\x22", mml_Octave_Down1),
        (b"\x81h", mml_Octave_Down1),
        (b"D%_", mml_Detune_Register_Relative),
        (b"D%", mml_Detune_Register),
        (b"D_", mml_Detune_Cent_Relative),
        (b"D", mml_Detune_Cent),
        (b"\x83f\x83B\x83`\x83\x85\x81[\x83\x93", mml_Detune_Cent),
        (b"__", mml_Transpose_Relative),
        (b"_", mml_Transpose),
        (b"k_", mml_KeyShift_Relative),
        (b"k", mml_KeyShift),
        (b"TrackKey", mml_Transpose),
        (b"P", mml_Protament),
        (b"\x83|\x83\x8b\x83^\x83\x81\x83\x93\x83g", mml_Protament),
        (b"{", mml_Protament2),
        (b"s", mml_Sweep),
        (b"v", mml_Volume),
        (b"\x89\xb9\x97\xca", mml_Volume),
        (b")", mml_Volume_Up),
        (b"\x91\xe5\x82\xab\x82\xad", mml_Volume_Up),
        (b"\x91\xe5", mml_Volume_Up),
        (b"(", mml_Volume_Down),
        (b"\x8f\xac\x82\xb3\x82\xad", mml_Volume_Down),
        (b"\x8f\xac", mml_Volume_Down),
        (b"yV", mml_VRC7_Write),
        (b"y", mml_Memory_Write),
        (b"#Priority", mml_Priority),
        (b"#priority", mml_Priority),
        (b"\x81@", mml_Bar),
        (b"|", mml_Bar),
        (b"\x81b", mml_Bar),
    ];
}

pub mod ks {
    pub const ks_Sharp: u32 = 0;
    pub const ks_Flat: u32 = 1;
    pub const ks_Natural: u32 = 2;
    pub const ks_c: u32 = 3;
    pub const ks_d: u32 = 4;
    pub const ks_e: u32 = 5;
    pub const ks_f: u32 = 6;
    pub const ks_g: u32 = 7;
    pub const ks_a: u32 = 8;
    pub const ks_b: u32 = 9;
    pub const ks_r: u32 = 10;
    pub const ks_0: u32 = 11;
    pub const ks_s1: u32 = 12;
    pub const ks_s2: u32 = 13;
    pub const ks_s3: u32 = 14;
    pub const ks_s4: u32 = 15;
    pub const ks_s5: u32 = 16;
    pub const ks_s6: u32 = 17;
    pub const ks_s7: u32 = 18;
    pub const ks_f1: u32 = 19;
    pub const ks_f2: u32 = 20;
    pub const ks_f3: u32 = 21;
    pub const ks_f4: u32 = 22;
    pub const ks_f5: u32 = 23;
    pub const ks_f6: u32 = 24;
    pub const ks_f7: u32 = 25;
    pub const ks_m0: u32 = 26;
    pub const ks_ms1: u32 = 27;
    pub const ks_ms2: u32 = 28;
    pub const ks_ms3: u32 = 29;
    pub const ks_ms4: u32 = 30;
    pub const ks_ms5: u32 = 31;
    pub const ks_ms6: u32 = 32;
    pub const ks_ms7: u32 = 33;
    pub const ks_ms8: u32 = 34;
    pub const ks_ms9: u32 = 35;
    pub const ks_mf1: u32 = 36;
    pub const ks_mf2: u32 = 37;
    pub const ks_mf3: u32 = 38;
    pub const ks_mf4: u32 = 39;
    pub const ks_mf5: u32 = 40;
    pub const ks_mf6: u32 = 41;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"C-Dur", ks_0),
        (b"Ds-Dur", ks_f5),
        (b"Des-Dur", ks_f5),
        (b"D-Dur", ks_s2),
        (b"Es-Dur", ks_f3),
        (b"E-Dur", ks_s4),
        (b"F-Dur", ks_f1),
        (b"Fis-Dur", ks_s6),
        (b"Gs-Dur", ks_f6),
        (b"Ges-Dur", ks_f6),
        (b"G-Dur", ks_s1),
        (b"As-Dur", ks_f4),
        (b"A-Dur", ks_s3),
        (b"B-Dur", ks_f2),
        (b"Bs-Dur", ks_f2),
        (b"H-Dur", ks_s5),
        (b"c-moll", ks_m0),
        (b"cis-moll", ks_ms7),
        (b"d-moll", ks_ms2),
        (b"dis-moll", ks_ms9),
        (b"es-moll", ks_mf3),
        (b"e-moll", ks_ms4),
        (b"f-moll", ks_mf1),
        (b"fis-moll", ks_ms6),
        (b"g-moll", ks_ms1),
        (b"gis-moll", ks_ms8),
        (b"a-moll", ks_ms3),
        (b"b-moll", ks_mf2),
        (b"bs-moll", ks_mf2),
        (b"h-moll", ks_ms5),
        (b"ds-moll", ks_mf5),
        (b"des-moll", ks_mf5),
        (b"gs-moll", ks_mf6),
        (b"ges-moll", ks_mf6),
        (b"as-moll", ks_mf4),
        (b"+", ks_Sharp),
        (b"\x81{", ks_Sharp),
        (b"#", ks_Sharp),
        (b"\x81\x94", ks_Sharp),
        (b"-", ks_Flat),
        (b"\x81|", ks_Flat),
        (b"\x81\xf3", ks_Flat),
        (b"=", ks_Natural),
        (b"\x81\x81", ks_Natural),
        (b"*", ks_Natural),
        (b"\x81\x96", ks_Natural),
        (b"c", ks_c),
        (b"d", ks_d),
        (b"e", ks_e),
        (b"f", ks_f),
        (b"g", ks_g),
        (b"a", ks_a),
        (b"b", ks_b),
        (b"r", ks_r),
        (b"\x83h", ks_c),
        (b"\x83\x8c", ks_d),
        (b"\x83~", ks_e),
        (b"\x83t\x83@", ks_f),
        (b"\x83t", ks_f),
        (b"\x83\x5c", ks_g),
        (b"\x83\x89", ks_a),
        (b"\x83V", ks_b),
        (b"\x83\x93", ks_r),
        (b"\x83b", ks_r),
        (b"\x82\xc7", ks_c),
        (b"\x82\xea", ks_d),
        (b"\x82\xdd", ks_e),
        (b"\x82\xd3\x82\x9f", ks_f),
        (b"\x82\xd3", ks_f),
        (b"\x82\xbb", ks_g),
        (b"\x82\xe7", ks_a),
        (b"\x82\xb5", ks_b),
        (b"\x82\xf1", ks_r),
        (b"\x82\xc1", ks_r),
        (b"\x83n", ks_c),
        (b"\x83j", ks_d),
        (b"\x83z", ks_e),
        (b"\x83w", ks_f),
        (b"\x83g", ks_g),
        (b"\x83C", ks_a),
        (b"\x83\x8d", ks_b),
    ];
}

pub mod env {
    pub const Env_Num: u32 = 0;
    pub const Env_Sweep: u32 = 1;
    pub const Env_Hold: u32 = 2;
    pub const Env_Loop: u32 = 3;
    pub const Env_Release: u32 = 4;
    pub const Env_Commma: u32 = 5;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"$", Env_Num),
        (b"%", Env_Num),
        (b"-", Env_Num),
        (b"0", Env_Num),
        (b"1", Env_Num),
        (b"2", Env_Num),
        (b"3", Env_Num),
        (b"4", Env_Num),
        (b"5", Env_Num),
        (b"6", Env_Num),
        (b"7", Env_Num),
        (b"8", Env_Num),
        (b"9", Env_Num),
        (b"{", Env_Sweep),
        (b"(", Env_Sweep),
        (b"d", Env_Hold),
        (b"D", Env_Hold),
        (b"\x82\x84", Env_Hold),
        (b"\x82c", Env_Hold),
        (b"|", Env_Loop),
        (b"\x81b", Env_Loop),
        (b"l", Env_Loop),
        (b"L", Env_Loop),
        (b"\x82\x8c", Env_Loop),
        (b"\x82k", Env_Loop),
        (b"r", Env_Release),
        (b"R", Env_Release),
        (b"\x82\x92", Env_Release),
        (b"\x82q", Env_Release),
        (b"\x81@", Env_Commma),
        (b"\x81C", Env_Commma),
        (b"\x81A", Env_Commma),
        (b",", Env_Commma),
    ];
}

pub mod patch {
    pub const Patch_MacroSet: u32 = 0;
    pub const Patch_Macro: u32 = 1;
    pub const Patch_C: u32 = 2;
    pub const Patch_Cis: u32 = 3;
    pub const Patch_D: u32 = 4;
    pub const Patch_Dis: u32 = 5;
    pub const Patch_E: u32 = 6;
    pub const Patch_F: u32 = 7;
    pub const Patch_Fis: u32 = 8;
    pub const Patch_G: u32 = 9;
    pub const Patch_Gis: u32 = 10;
    pub const Patch_A: u32 = 11;
    pub const Patch_Ais: u32 = 12;
    pub const Patch_B: u32 = 13;
    pub const Patch_Note: u32 = 14;
    pub const Patch_Off_Evoi: u32 = 15;
    pub const Patch_Off_Evol: u32 = 16;
    pub const Patch_Off_Em: u32 = 17;
    pub const Patch_Off_En: u32 = 18;
    pub const Patch_Evoi: u32 = 19;
    pub const Patch_Evol: u32 = 20;
    pub const Patch_Em: u32 = 21;
    pub const Patch_En: u32 = 22;
    pub const Patch_Gate_q: u32 = 23;
    pub const Patch_Gate_u: u32 = 24;
    pub const Patch_Voice: u32 = 25;
    pub const Patch_KeyShift: u32 = 26;
    pub const Patch_n163set: u32 = 27;
    pub const Patch_Sweep: u32 = 28;
    pub const Patch_Sub: u32 = 29;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"$$", Patch_MacroSet),
        (b"$", Patch_Macro),
        (b"c#", Patch_Cis),
        (b"d#", Patch_Dis),
        (b"f#", Patch_Fis),
        (b"g#", Patch_Gis),
        (b"a#", Patch_Ais),
        (b"c+", Patch_Cis),
        (b"d+", Patch_Dis),
        (b"f+", Patch_Fis),
        (b"g+", Patch_Gis),
        (b"a+", Patch_Ais),
        (b"d-", Patch_Cis),
        (b"e-", Patch_Dis),
        (b"g-", Patch_Fis),
        (b"a-", Patch_Gis),
        (b"b-", Patch_Ais),
        (b"c", Patch_C),
        (b"d", Patch_D),
        (b"e", Patch_E),
        (b"f", Patch_F),
        (b"g", Patch_G),
        (b"a", Patch_A),
        (b"b", Patch_B),
        (b"\x82\xc7#", Patch_Cis),
        (b"\x82\xea#", Patch_Dis),
        (b"\x82\xd3\x82\x9f#", Patch_Fis),
        (b"\x82\xd3#", Patch_Fis),
        (b"\x82\xbb#", Patch_Gis),
        (b"\x82\xe7#", Patch_Ais),
        (b"\x82\xea-", Patch_Cis),
        (b"\x82\xdd-", Patch_Dis),
        (b"\x82\xbb-", Patch_Fis),
        (b"\x82\xe7-", Patch_Gis),
        (b"\x82\xb5-", Patch_Ais),
        (b"\x82\xc7", Patch_C),
        (b"\x82\xea", Patch_D),
        (b"\x82\xdd", Patch_E),
        (b"\x82\xd3\x82\x9f", Patch_F),
        (b"\x82\xd3", Patch_F),
        (b"\x82\xbb", Patch_G),
        (b"\x82\xe7", Patch_A),
        (b"\x82\xb5", Patch_B),
        (b"\x83h#", Patch_Cis),
        (b"\x83\x8c#", Patch_Dis),
        (b"\x83t\x83@#", Patch_Fis),
        (b"\x83t#", Patch_Fis),
        (b"\x83\x5c#", Patch_Gis),
        (b"\x83\x89#", Patch_Ais),
        (b"\x83\x8c-", Patch_Cis),
        (b"\x83~-", Patch_Dis),
        (b"\x83\x5c-", Patch_Fis),
        (b"\x83\x89-", Patch_Gis),
        (b"\x83V-", Patch_Ais),
        (b"\x83h", Patch_C),
        (b"\x83\x8c", Patch_D),
        (b"\x83~", Patch_E),
        (b"\x83t\x83@", Patch_F),
        (b"\x83t", Patch_F),
        (b"\x83\x5c", Patch_G),
        (b"\x83\x89", Patch_A),
        (b"\x83V", Patch_B),
        (b"n", Patch_Note),
        (b"E@*", Patch_Off_Evoi),
        (b"Ev*", Patch_Off_Evol),
        (b"Em*", Patch_Off_Em),
        (b"En*", Patch_Off_En),
        (b"E@", Patch_Evoi),
        (b"Ev", Patch_Evol),
        (b"Em", Patch_Em),
        (b"En", Patch_En),
        (b"q", Patch_Gate_q),
        (b"u", Patch_Gate_u),
        (b"s", Patch_Sweep),
        (b"@NS", Patch_n163set),
        (b"@", Patch_Voice),
        (b"_", Patch_KeyShift),
        (b"S", Patch_Sub),
    ];
}

pub mod dpcm {
    pub const DPCM_C: u32 = 0;
    pub const DPCM_Cis: u32 = 1;
    pub const DPCM_D: u32 = 2;
    pub const DPCM_Dis: u32 = 3;
    pub const DPCM_E: u32 = 4;
    pub const DPCM_F: u32 = 5;
    pub const DPCM_Fis: u32 = 6;
    pub const DPCM_G: u32 = 7;
    pub const DPCM_Gis: u32 = 8;
    pub const DPCM_A: u32 = 9;
    pub const DPCM_Ais: u32 = 10;
    pub const DPCM_B: u32 = 11;
    pub const DPCM_Note: u32 = 12;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"c#", DPCM_Cis),
        (b"d#", DPCM_Dis),
        (b"f#", DPCM_Fis),
        (b"g#", DPCM_Gis),
        (b"a#", DPCM_Ais),
        (b"c+", DPCM_Cis),
        (b"d+", DPCM_Dis),
        (b"f+", DPCM_Fis),
        (b"g+", DPCM_Gis),
        (b"a+", DPCM_Ais),
        (b"d-", DPCM_Cis),
        (b"e-", DPCM_Dis),
        (b"g-", DPCM_Fis),
        (b"a-", DPCM_Gis),
        (b"b-", DPCM_Ais),
        (b"c", DPCM_C),
        (b"d", DPCM_D),
        (b"e", DPCM_E),
        (b"f", DPCM_F),
        (b"g", DPCM_G),
        (b"a", DPCM_A),
        (b"b", DPCM_B),
        (b"\x82\xc7#", DPCM_Cis),
        (b"\x82\xea#", DPCM_Dis),
        (b"\x82\xd3\x82\x9f#", DPCM_Fis),
        (b"\x82\xd3#", DPCM_Fis),
        (b"\x82\xbb#", DPCM_Gis),
        (b"\x82\xe7#", DPCM_Ais),
        (b"\x82\xea-", DPCM_Cis),
        (b"\x82\xdd-", DPCM_Dis),
        (b"\x82\xbb-", DPCM_Fis),
        (b"\x82\xe7-", DPCM_Gis),
        (b"\x82\xb5-", DPCM_Ais),
        (b"\x82\xc7", DPCM_C),
        (b"\x82\xea", DPCM_D),
        (b"\x82\xdd", DPCM_E),
        (b"\x82\xd3\x82\x9f", DPCM_F),
        (b"\x82\xd3", DPCM_F),
        (b"\x82\xbb", DPCM_G),
        (b"\x82\xe7", DPCM_A),
        (b"\x82\xb5", DPCM_B),
        (b"\x83h#", DPCM_Cis),
        (b"\x83\x8c#", DPCM_Dis),
        (b"\x83t\x83@#", DPCM_Fis),
        (b"\x83t#", DPCM_Fis),
        (b"\x83\x5c#", DPCM_Gis),
        (b"\x83\x89#", DPCM_Ais),
        (b"\x83\x8c-", DPCM_Cis),
        (b"\x83~-", DPCM_Dis),
        (b"\x83\x5c-", DPCM_Fis),
        (b"\x83\x89-", DPCM_Gis),
        (b"\x83V-", DPCM_Ais),
        (b"\x83h", DPCM_C),
        (b"\x83\x8c", DPCM_D),
        (b"\x83~", DPCM_E),
        (b"\x83t\x83@", DPCM_F),
        (b"\x83t", DPCM_F),
        (b"\x83\x5c", DPCM_G),
        (b"\x83\x89", DPCM_A),
        (b"\x83V", DPCM_B),
        (b"n", DPCM_Note),
    ];
}

pub mod vrc7 {
    pub const VRC7_Normal: u32 = 0;
    pub const VRC7_Resister: u32 = 1;
    pub const VRC7_Num: u32 = 2;
    pub const VRC7_Comma: u32 = 3;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"@R", VRC7_Resister),
        (b"@", VRC7_Normal),
        (b"0", VRC7_Num),
        (b"1", VRC7_Num),
        (b"2", VRC7_Num),
        (b"3", VRC7_Num),
        (b"4", VRC7_Num),
        (b"5", VRC7_Num),
        (b"6", VRC7_Num),
        (b"7", VRC7_Num),
        (b"8", VRC7_Num),
        (b"9", VRC7_Num),
        (b"$", VRC7_Num),
        (b"%", VRC7_Num),
        (b",", VRC7_Comma),
    ];
}

pub mod fdsc {
    pub const FDSC_Num: u32 = 0;
    pub const FDSC_Commma: u32 = 1;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"%", FDSC_Num),
        (b"$", FDSC_Num),
        (b"0", FDSC_Num),
        (b"1", FDSC_Num),
        (b"2", FDSC_Num),
        (b"3", FDSC_Num),
        (b"4", FDSC_Num),
        (b"5", FDSC_Num),
        (b"6", FDSC_Num),
        (b"7", FDSC_Num),
        (b"8", FDSC_Num),
        (b"9", FDSC_Num),
        (b",", FDSC_Commma),
    ];
}

pub mod fdsm {
    pub const FDSM_Num: u32 = 0;
    pub const FDSM_Commma: u32 = 1;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"%", FDSM_Num),
        (b"$", FDSM_Num),
        (b"0", FDSM_Num),
        (b"1", FDSM_Num),
        (b"2", FDSM_Num),
        (b"3", FDSM_Num),
        (b"4", FDSM_Num),
        (b"5", FDSM_Num),
        (b"6", FDSM_Num),
        (b"7", FDSM_Num),
        (b"8", FDSM_Num),
        (b"9", FDSM_Num),
        (b",", FDSM_Commma),
    ];
}

pub mod n163 {
    pub const N163_Num: u32 = 0;
    pub const N163_Commma: u32 = 1;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"%", N163_Num),
        (b"$", N163_Num),
        (b"0", N163_Num),
        (b"1", N163_Num),
        (b"2", N163_Num),
        (b"3", N163_Num),
        (b"4", N163_Num),
        (b"5", N163_Num),
        (b"6", N163_Num),
        (b"7", N163_Num),
        (b"8", N163_Num),
        (b"9", N163_Num),
        (b",", N163_Commma),
    ];
}

pub mod plst {
    pub const plst_Num: u32 = 0;
    pub const plst_Commma: u32 = 1;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"%", plst_Num),
        (b"$", plst_Num),
        (b"0", plst_Num),
        (b"1", plst_Num),
        (b"2", plst_Num),
        (b"3", plst_Num),
        (b"4", plst_Num),
        (b"5", plst_Num),
        (b"6", plst_Num),
        (b"7", plst_Num),
        (b"8", plst_Num),
        (b"9", plst_Num),
        (b",", plst_Commma),
    ];
}

pub mod psfx {
    pub const psfx_Num: u32 = 0;
    pub const psfx_Commma: u32 = 1;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"%", psfx_Num),
        (b"$", psfx_Num),
        (b"0", psfx_Num),
        (b"1", psfx_Num),
        (b"2", psfx_Num),
        (b"3", psfx_Num),
        (b"4", psfx_Num),
        (b"5", psfx_Num),
        (b"6", psfx_Num),
        (b"7", psfx_Num),
        (b"8", psfx_Num),
        (b"9", psfx_Num),
        (b",", psfx_Commma),
    ];
}

pub mod mixe {
    pub const mixe_APU1: u32 = 0;
    pub const mixe_APU2: u32 = 1;
    pub const mixe_VRC6: u32 = 2;
    pub const mixe_VRC7: u32 = 3;
    pub const mixe_FDS: u32 = 4;
    pub const mixe_MMC5: u32 = 5;
    pub const mixe_N163: u32 = 6;
    pub const mixe_SN5B: u32 = 7;
    pub const mixe_Num: u32 = 8;
    pub const mixe_Commma: u32 = 9;
    pub const TABLE: &[(&[u8], u32)] = &[
        (b"APU1", mixe_APU1),
        (b"APU2", mixe_APU2),
        (b"VRC6", mixe_VRC6),
        (b"VRC7", mixe_VRC7),
        (b"FDS", mixe_FDS),
        (b"MMC5", mixe_MMC5),
        (b"N160", mixe_N163),
        (b"N163", mixe_N163),
        (b"N16x", mixe_N163),
        (b"SN5B", mixe_SN5B),
        (b"PSG", mixe_SN5B),
        (b"%", mixe_Num),
        (b"$", mixe_Num),
        (b"-", mixe_Num),
        (b"0", mixe_Num),
        (b"1", mixe_Num),
        (b"2", mixe_Num),
        (b"3", mixe_Num),
        (b"4", mixe_Num),
        (b"5", mixe_Num),
        (b"6", mixe_Num),
        (b"7", mixe_Num),
        (b"8", mixe_Num),
        (b"9", mixe_Num),
        (b"=", mixe_Commma),
        (b",", mixe_Commma),
    ];
}
