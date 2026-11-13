// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! Every BGN generator a scene can use: its name, its group, the layer it
//! usually goes in and its parameters, so a UI can build its controls
//! from this alone. `gens.rs` makes the sounds, reading the parameters in
//! the order they're listed here.

/// What a parameter's 0..1 slider stands for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// From `lo` to `hi`, exponentially when `exp` (frequencies, rates).
    Range { lo: f32, hi: f32, exp: bool, unit: &'static str },
    /// One of these, stored as its index.
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    /// 0..1 for a range, the index for a choice.
    pub default: f32,
}

impl Param {
    /// The value a generator works with, from the stored one.
    pub fn value(&self, p: f32) -> f32 {
        match self.kind {
            Kind::Range { lo, hi, exp, .. } => {
                let p = p.clamp(0.0, 1.0);
                if exp {
                    lo * (hi / lo).powf(p)
                } else {
                    lo + (hi - lo) * p
                }
            }
            Kind::Choice(options) => p.round().clamp(0.0, (options.len() - 1) as f32),
        }
    }

    /// The value as a person reads it ("120 Hz", "Pink").
    pub fn describe(&self, p: f32) -> String {
        match self.kind {
            Kind::Range { unit: "%", .. } => format!("{:.0}%", self.value(p) * 100.0),
            Kind::Range { unit, .. } => {
                let v = self.value(p);
                let n = if v.abs() >= 100.0 {
                    format!("{v:.0}")
                } else if v.abs() >= 10.0 {
                    format!("{v:.1}")
                } else {
                    format!("{v:.2}")
                };
                if unit.is_empty() {
                    n
                } else {
                    format!("{n} {unit}")
                }
            }
            Kind::Choice(options) => options[self.value(p) as usize].to_string(),
        }
    }

    pub fn is_choice(&self) -> bool {
        matches!(self.kind, Kind::Choice(_))
    }
}

const fn r(name: &'static str, lo: f32, hi: f32, unit: &'static str, default: f32) -> Param {
    Param { name, kind: Kind::Range { lo, hi, exp: false, unit }, default }
}

const fn e(name: &'static str, lo: f32, hi: f32, unit: &'static str, default: f32) -> Param {
    Param { name, kind: Kind::Range { lo, hi, exp: true, unit }, default }
}

/// A 0..1 amount, shown as a percentage.
const fn a(name: &'static str, default: f32) -> Param {
    Param { name, kind: Kind::Range { lo: 0.0, hi: 1.0, exp: false, unit: "%" }, default }
}

const fn c(name: &'static str, options: &'static [&'static str], default: f32) -> Param {
    Param { name, kind: Kind::Choice(options), default }
}

/// The four layers a scene is built from, in order.
pub const LAYERS: [&str; 4] = ["Bed", "Texture", "Details", "Movement"];

pub const BED: usize = 0;
pub const TEXTURE: usize = 1;
pub const DETAILS: usize = 2;
pub const MOVEMENT: usize = 3;

pub struct Generator {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// The layer it usually belongs in.
    pub layer: usize,
    pub about: &'static str,
    pub params: &'static [Param],
}

impl Generator {
    pub fn defaults(&self) -> [f32; 6] {
        let mut p = [0.0; 6];
        for (i, q) in self.params.iter().enumerate() {
            p[i] = q.default;
        }
        p
    }
}

pub fn generator(id: &str) -> Option<&'static Generator> {
    GENERATORS.iter().find(|g| g.id == id)
}

/// The groups, in the order a UI lists them.
pub const GROUPS: [&str; 10] = ["Noise", "Weather", "Water", "Fire & earth", "Wildlife", "People", "Machines", "Vehicles", "Electronic & tonal", "Signals"];

const LOW_CUT: Param = e("Low cut", 10.0, 4000.0, "Hz", 0.0);
const HIGH_CUT: Param = e("High cut", 100.0, 20000.0, "Hz", 1.0);
const TONE: Param = a("Tone", 0.5);
const DISTANCE: Param = a("Distance", 0.3);

pub static GENERATORS: &[Generator] = &[
    // Noise: every method noise.rs has.
    Generator {
        id: "noise",
        name: "Noise",
        group: "Noise",
        layer: BED,
        about: "Colored noise: white, Gaussian, pink three ways, brown, blue, violet, grey or velvet.",
        params: &[
            c("Color", &["White", "Gaussian", "Pink", "Pink (Voss-McCartney)", "Pink (3-pole)", "Brown", "Blue", "Violet", "Grey", "Velvet"], 2.0),
            LOW_CUT,
            HIGH_CUT,
            a("Drift", 0.2),
            e("Pulses", 100.0, 20000.0, "/s", 0.6),
        ],
    },
    Generator {
        id: "spectral",
        name: "Spectral noise",
        group: "Noise",
        layer: BED,
        about: "Noise shaped frame by frame in the frequency domain, to any slope, band and peak.",
        params: &[
            r("Slope", -12.0, 6.0, "dB/oct", 0.5),
            LOW_CUT,
            HIGH_CUT,
            e("Peak", 40.0, 12000.0, "Hz", 0.5),
            r("Peak gain", -24.0, 24.0, "dB", 0.5),
            r("Peak width", 0.1, 4.0, "oct", 0.25),
        ],
    },
    Generator {
        id: "digital",
        name: "Digital noise",
        group: "Noise",
        layer: BED,
        about: "Shift-register noise as old sound chips made it, clocked and bit-crushed.",
        params: &[
            c("Register", &["15-bit long", "15-bit short", "7-bit", "23-bit, 8 out", "17-bit poly", "4-bit buzz"], 0.0),
            e("Clock", 50.0, 400000.0, "Hz", 0.75),
            r("Bits", 1.0, 16.0, "", 1.0),
            TONE,
        ],
    },
    Generator {
        id: "random",
        name: "Random steps",
        group: "Noise",
        layer: BED,
        about: "Sample-and-hold noise: new values held, ramped or curved between.",
        params: &[c("Shape", &["Stepped", "Linear", "Cubic"], 0.0), e("Rate", 2.0, 20000.0, "Hz", 0.6), TONE, a("Level wander", 0.0)],
    },
    Generator {
        id: "chaos",
        name: "Chaotic noise",
        group: "Noise",
        layer: BED,
        about: "Noise from chaos: the logistic and Henon maps, Lorenz's system, crackle and popcorn noise.",
        params: &[c("System", &["Logistic", "Henon", "Lorenz", "Crackle", "Popcorn"], 0.0), e("Rate", 20.0, 44100.0, "Hz", 0.7), a("Chaos", 0.7), TONE],
    },
    Generator {
        id: "crackle",
        name: "Crackle",
        group: "Noise",
        layer: TEXTURE,
        about: "Pops and ticks on a hiss: vinyl, fire, electricity or old film.",
        params: &[c("Kind", &["Vinyl", "Fire", "Electrical", "Old film"], 0.0), e("Density", 0.5, 300.0, "/s", 0.4), a("Size", 0.5), a("Hiss", 0.3), TONE],
    },
    Generator {
        id: "static",
        name: "Radio static",
        group: "Noise",
        layer: BED,
        about: "Between stations: hiss, crackle, heterodyne whistles and fading.",
        params: &[c("Kind", &["Radio static", "Shortwave", "TV snow"], 0.0), a("Hiss", 0.7), a("Crackle", 0.4), a("Whistles", 0.3), a("Fading", 0.4), TONE],
    },
    // Weather.
    Generator {
        id: "wind",
        name: "Wind",
        group: "Weather",
        layer: BED,
        about: "Moving air: band-passed noise that swells and gusts, with whistling where it meets edges.",
        params: &[a("Strength", 0.4), a("Gusts", 0.4), a("Whistle", 0.15), e("Pitch", 100.0, 2000.0, "Hz", 0.4), TONE, e("Gust speed", 0.02, 1.0, "Hz", 0.35)],
    },
    Generator {
        id: "foliage",
        name: "Leaves & grass",
        group: "Weather",
        layer: TEXTURE,
        about: "The rustle of leaves, needles or grass as wind moves them.",
        params: &[a("Amount", 0.5), c("Plant", &["Broad leaves", "Pines", "Grass", "Dry leaves", "Bamboo", "Reeds"], 0.0), a("Gusts", 0.5), TONE],
    },
    Generator {
        id: "rain",
        name: "Rain",
        group: "Weather",
        layer: TEXTURE,
        about: "A wash of rain and the drops nearest you, on the surface they're falling on.",
        params: &[
            a("Intensity", 0.5),
            a("Drop size", 0.4),
            c("Surface", &["Ground", "Leaves", "Tin roof", "Tile roof", "Window", "Tent", "Umbrella", "Car roof", "Puddles", "Gutter", "Snow"], 0.0),
            a("Closeness", 0.5),
            TONE,
        ],
    },
    Generator {
        id: "thunder",
        name: "Thunder",
        group: "Weather",
        layer: DETAILS,
        about: "Claps and long rolls of thunder, near or far.",
        params: &[e("Rate", 0.2, 12.0, "/min", 0.4), DISTANCE, a("Power", 0.6), a("Length", 0.5)],
    },
    // Water.
    Generator {
        id: "stream",
        name: "Stream & river",
        group: "Water",
        layer: TEXTURE,
        about: "Running water as a crowd of bubbles over a rush, from a trickle to rapids.",
        params: &[a("Flow", 0.4), a("Bubbles", 0.6), a("Pitch", 0.5), a("Turbulence", 0.4), TONE],
    },
    Generator {
        id: "surf",
        name: "Ocean surf",
        group: "Water",
        layer: TEXTURE,
        about: "Waves that build, break and draw back over sand, shingle or rock.",
        params: &[a("Size", 0.5), r("Period", 4.0, 18.0, "s", 0.4), a("Crash", 0.5), c("Shore", &["Sand", "Shingle", "Rocks", "Open sea"], 0.0), TONE],
    },
    Generator {
        id: "lapping",
        name: "Lapping water",
        group: "Water",
        layer: TEXTURE,
        about: "Small waves against a shore, a hull or a dock's pilings.",
        params: &[e("Rate", 0.2, 4.0, "/s", 0.4), a("Size", 0.4), a("Hollow", 0.3), TONE],
    },
    Generator {
        id: "torrent",
        name: "Waterfall",
        group: "Water",
        layer: BED,
        about: "Falling water's roar: rumble below, spray above.",
        params: &[a("Power", 0.6), DISTANCE, a("Spray", 0.5), a("Rumble", 0.5)],
    },
    Generator {
        id: "drips",
        name: "Drips",
        group: "Water",
        layer: DETAILS,
        about: "Single drops falling into water or onto stone.",
        params: &[e("Rate", 0.1, 20.0, "/s", 0.35), a("Pitch", 0.5), a("Regularity", 0.2), a("Size", 0.5), c("Into", &["Water", "Stone", "Metal", "Puddle"], 0.0)],
    },
    Generator {
        id: "bubbling",
        name: "Bubbling",
        group: "Water",
        layer: TEXTURE,
        about: "Thick bubbling: mud, a swamp, a potion, lava, a hot spring.",
        params: &[e("Rate", 0.5, 60.0, "/s", 0.4), a("Size", 0.5), a("Thickness", 0.5), a("Pitch", 0.5)],
    },
    Generator {
        id: "underwater",
        name: "Underwater",
        group: "Water",
        layer: BED,
        about: "Below the surface: muffled pressure, drifting bubbles, the far murmur of the sea.",
        params: &[a("Depth", 0.5), a("Bubbles", 0.3), a("Throb", 0.3), a("Murmur", 0.5)],
    },
    Generator {
        id: "ice",
        name: "Ice",
        group: "Water",
        layer: DETAILS,
        about: "Ice under strain: the zinging cracks of a frozen lake, a glacier's groans.",
        params: &[e("Rate", 0.05, 5.0, "/s", 0.3), a("Size", 0.5), a("Ring", 0.6), c("Kind", &["Lake ice", "Glacier", "Icicles"], 0.0)],
    },
    // Fire and earth.
    Generator {
        id: "fire",
        name: "Fire",
        group: "Fire & earth",
        layer: TEXTURE,
        about: "Burning: a hiss, the lapping roar of flames and the crackle of wood.",
        params: &[a("Size", 0.4), a("Crackle", 0.5), a("Hiss", 0.4), a("Roar", 0.3)],
    },
    Generator {
        id: "rumble",
        name: "Rumble",
        group: "Fire & earth",
        layer: BED,
        about: "Sub-bass: distant traffic, a volcano, a ship's engines, dread.",
        params: &[e("Depth", 20.0, 200.0, "Hz", 0.4), a("Throb", 0.2), e("Throb rate", 0.05, 8.0, "Hz", 0.3), a("Power", 0.5)],
    },
    Generator {
        id: "steam",
        name: "Steam & hiss",
        group: "Fire & earth",
        layer: TEXTURE,
        about: "Gas under pressure: vents, pipes, geysers, pneumatics.",
        params: &[a("Pressure", 0.5), e("Bursts", 0.02, 3.0, "/s", 0.3), TONE, a("Gurgle", 0.2)],
    },
    // Wildlife.
    Generator {
        id: "crickets",
        name: "Crickets",
        group: "Wildlife",
        layer: TEXTURE,
        about: "Crickets and katydids, chirping faster the warmer it is.",
        params: &[a("Density", 0.5), r("Temperature", 10.0, 35.0, "C", 0.5), a("Pitch", 0.5), a("Sync", 0.2), c("Kind", &["Field crickets", "Tree crickets", "Katydids"], 0.0)],
    },
    Generator {
        id: "cicadas",
        name: "Cicadas",
        group: "Wildlife",
        layer: TEXTURE,
        about: "The buzzing swell of cicadas in summer heat.",
        params: &[a("Density", 0.5), a("Pitch", 0.5), a("Swell", 0.5), a("Harshness", 0.4)],
    },
    Generator {
        id: "frogs",
        name: "Frogs",
        group: "Wildlife",
        layer: TEXTURE,
        about: "Peepers, bullfrogs, tree frogs and toads calling.",
        params: &[c("Kind", &["Peepers", "Bullfrogs", "Tree frogs", "Toads"], 0.0), a("Density", 0.5), a("Pitch", 0.5), DISTANCE],
    },
    Generator {
        id: "insects",
        name: "Buzzing insects",
        group: "Wildlife",
        layer: DETAILS,
        about: "Flies, mosquitoes and bees buzzing past, or a swarm.",
        params: &[c("Kind", &["Flies", "Mosquito", "Bees", "Swarm", "Hive"], 0.0), a("Count", 0.3), a("Closeness", 0.5), a("Pitch", 0.5)],
    },
    Generator {
        id: "birds",
        name: "Birds",
        group: "Wildlife",
        layer: DETAILS,
        about: "Birdsong and calls, from a dawn chorus to gulls and crows.",
        params: &[
            c("Kind", &["Songbirds", "Sparrows", "Dawn chorus", "Gulls", "Crows", "Owls", "Doves", "Tropical", "Woodpecker", "Loons", "Parrots", "Hawk"], 0.0),
            e("Density", 0.05, 8.0, "/s", 0.4),
            a("Pitch", 0.5),
            a("Variety", 0.5),
            DISTANCE,
        ],
    },
    Generator {
        id: "calls",
        name: "Animal calls",
        group: "Wildlife",
        layer: DETAILS,
        about: "Voiced calls: howls, barks, lowing, bleats, whale song.",
        params: &[
            c("Animal", &["Wolf", "Coyote", "Dog", "Whale", "Cow", "Sheep", "Goat", "Horse", "Monkey", "Elephant", "Lion", "Seal", "Rooster", "Pig", "Hyena"], 0.0),
            e("Rate", 0.01, 2.0, "/s", 0.3),
            a("Pitch", 0.5),
            DISTANCE,
        ],
    },
    // People.
    Generator {
        id: "crowd",
        name: "Crowd walla",
        group: "People",
        layer: TEXTURE,
        about: "People talking where no one word can be made out, from a few to a stadium.",
        params: &[e("People", 1.0, 400.0, "", 0.45), a("Excitement", 0.3), a("Pitch", 0.5), DISTANCE, a("Children", 0.0)],
    },
    Generator {
        id: "applause",
        name: "Applause & cheers",
        group: "People",
        layer: TEXTURE,
        about: "Clapping, cheering, chanting, booing, laughing.",
        params: &[c("Kind", &["Applause", "Cheering", "Chanting", "Booing", "Laughter"], 0.0), a("Amount", 0.5), a("Intensity", 0.5), a("Swell", 0.4)],
    },
    Generator {
        id: "footsteps",
        name: "Footsteps",
        group: "People",
        layer: DETAILS,
        about: "People walking by on stone, wood, gravel or snow.",
        params: &[c("Surface", &["Hard", "Wood", "Gravel", "Snow", "Carpet", "Metal", "Mud"], 0.0), e("People", 1.0, 40.0, "", 0.2), a("Pace", 0.5), DISTANCE],
    },
    Generator {
        id: "clatter",
        name: "Dishes & clinks",
        group: "People",
        layer: DETAILS,
        about: "Glasses, plates, cutlery and pans: kitchens, bars, restaurants.",
        params: &[c("Material", &["Glass", "Ceramic", "Cutlery", "Pans", "Coins", "Chips"], 0.0), e("Rate", 0.1, 20.0, "/s", 0.4), a("Size", 0.5), DISTANCE],
    },
    Generator {
        id: "typing",
        name: "Typing & clicks",
        group: "People",
        layer: DETAILS,
        about: "Keyboards, typewriters, teletypes and mice.",
        params: &[c("Kind", &["Keyboard", "Mechanical", "Typewriter", "Teletype", "Mouse clicks"], 0.0), a("Rate", 0.5), e("Typists", 1.0, 30.0, "", 0.1), DISTANCE],
    },
    Generator {
        id: "choir",
        name: "Voices pad",
        group: "People",
        layer: BED,
        about: "Sustained voices: chant, a distant choir, a celestial pad.",
        params: &[e("Pitch", 55.0, 880.0, "Hz", 0.4), c("Vowel", &["Ah", "Eh", "Ee", "Oh", "Oo", "Hum"], 0.0), e("Voices", 1.0, 24.0, "", 0.5), a("Motion", 0.4), c("Chord", &["Unison", "Fifth", "Octaves", "Minor", "Major", "Cluster"], 1.0)],
    },
    Generator {
        id: "whispers",
        name: "Whispers",
        group: "People",
        layer: DETAILS,
        about: "Indistinct whispering voices, close or all around.",
        params: &[e("Density", 0.05, 4.0, "/s", 0.3), a("Closeness", 0.5), a("Pitch", 0.5), a("Breath", 0.8)],
    },
    Generator {
        id: "breathing",
        name: "Breathing",
        group: "People",
        layer: TEXTURE,
        about: "Breath, calm or ragged, in the open or in a helmet.",
        params: &[e("Rate", 4.0, 40.0, "/min", 0.35), a("Depth", 0.5), c("Kind", &["Calm", "Heavy", "Helmet", "Respirator"], 0.0), a("Closeness", 0.6)],
    },
    Generator {
        id: "heartbeat",
        name: "Heartbeat",
        group: "People",
        layer: TEXTURE,
        about: "A heart beating, from resting to racing.",
        params: &[r("Rate", 40.0, 180.0, "bpm", 0.2), a("Strength", 0.6), a("Muffle", 0.5)],
    },
    Generator {
        id: "music",
        name: "Distant music",
        group: "People",
        layer: TEXTURE,
        about: "Music from somewhere else: through a wall, across a square, on an old radio.",
        params: &[
            c("Style", &["Club beat", "Bar band", "Piano", "Organ", "Radio", "Music box", "Brass band", "War drums", "Folk", "Carnival"], 0.0),
            r("Tempo", 50.0, 160.0, "bpm", 0.5),
            a("Muffle", 0.6),
            a("Detune", 0.0),
        ],
    },
    Generator {
        id: "knocks",
        name: "Knocks & bangs",
        group: "People",
        layer: DETAILS,
        about: "Knocks, thuds, footsteps overhead, doors and distant bangs.",
        params: &[c("Kind", &["Knock", "Thud", "Upstairs", "Door slam", "Metal bang", "Hammering"], 0.0), e("Rate", 0.01, 3.0, "/s", 0.3), a("Size", 0.5), DISTANCE],
    },
    // Machines.
    Generator {
        id: "hum",
        name: "Electrical hum",
        group: "Machines",
        layer: BED,
        about: "Mains hum and transformer buzz, the whine of fluorescent tubes.",
        params: &[c("Mains", &["50 Hz", "60 Hz"], 1.0), a("Harmonics", 0.4), a("Buzz", 0.2), a("Wobble", 0.1), a("Whine", 0.0)],
    },
    Generator {
        id: "air",
        name: "Air & fans",
        group: "Machines",
        layer: BED,
        about: "Moving air: vents, fans, server rooms, a fridge's compressor.",
        params: &[c("Kind", &["Air vent", "Fan", "Server room", "Fridge", "Hair dryer"], 0.0), a("Airflow", 0.5), a("Blade tone", 0.3), a("Speed", 0.5), a("Rattle", 0.1)],
    },
    Generator {
        id: "machine",
        name: "Machinery",
        group: "Machines",
        layer: TEXTURE,
        about: "Machines at work: presses, looms, pumps, conveyors, projectors.",
        params: &[
            c("Kind", &["Factory", "Press", "Loom", "Pump", "Conveyor", "Projector", "Printing press", "Clockwork"], 0.0),
            e("Rate", 0.3, 30.0, "Hz", 0.4),
            a("Weight", 0.5),
            a("Whine", 0.3),
            a("Hiss", 0.2),
            a("Irregularity", 0.2),
        ],
    },
    Generator {
        id: "engine",
        name: "Engine",
        group: "Vehicles",
        layer: TEXTURE,
        about: "A motor running: cars, diesels, motorbikes, props, jets, generators, steam.",
        params: &[
            c("Kind", &["Car", "Diesel", "Motorbike", "Prop plane", "Jet", "Ship", "Steam", "Generator"], 0.0),
            e("RPM", 400.0, 9000.0, "", 0.2),
            a("Load", 0.4),
            a("Roughness", 0.3),
            a("Inside", 0.0),
        ],
    },
    Generator {
        id: "road",
        name: "Tires on road",
        group: "Vehicles",
        layer: BED,
        about: "Road noise from inside or beside a car, dry or wet.",
        params: &[a("Speed", 0.5), a("Wet", 0.0), a("Roughness", 0.4), a("Inside", 1.0)],
    },
    Generator {
        id: "traffic",
        name: "Traffic",
        group: "Vehicles",
        layer: TEXTURE,
        about: "A city's traffic, from one street to a highway's drone.",
        params: &[a("Density", 0.5), DISTANCE, a("Speed", 0.4), a("Wet", 0.0), a("Heavy", 0.3)],
    },
    Generator {
        id: "rail",
        name: "Train",
        group: "Vehicles",
        layer: TEXTURE,
        about: "Wheels on rails: the clack of the joints, the rumble, the squeal on curves.",
        params: &[c("Kind", &["Passenger", "Freight", "Subway", "Steam", "Tram", "High speed"], 0.0), a("Speed", 0.5), a("Clacks", 0.5), a("Squeal", 0.1), a("Inside", 1.0)],
    },
    Generator {
        id: "aircraft",
        name: "Aircraft",
        group: "Vehicles",
        layer: BED,
        about: "In or near an aircraft: airliner cabin, cockpit, helicopter, airship.",
        params: &[c("Kind", &["Airliner cabin", "Prop cockpit", "Helicopter", "Jet cockpit", "Airship", "Hot air balloon"], 0.0), a("Power", 0.5), a("Inside", 1.0), TONE],
    },
    Generator {
        id: "creaks",
        name: "Creaks",
        group: "Machines",
        layer: DETAILS,
        about: "Wood and rope and hinges under strain: old houses, ships, swings.",
        params: &[c("Kind", &["House", "Ship timbers", "Rigging", "Door", "Swing", "Metal groan", "Rocking chair"], 0.0), e("Rate", 0.02, 3.0, "/s", 0.3), a("Pitch", 0.5), a("Length", 0.5)],
    },
    Generator {
        id: "rattle",
        name: "Rattles",
        group: "Machines",
        layer: DETAILS,
        about: "Many small collisions: gravel, chains, shutters, debris, sand.",
        params: &[c("Kind", &["Gravel", "Chains", "Shutters", "Debris", "Sand", "Bones"], 0.0), e("Rate", 0.02, 4.0, "/s", 0.3), a("Intensity", 0.5), a("Size", 0.5)],
    },
    // Electronic and tonal.
    Generator {
        id: "beeps",
        name: "Beeps & consoles",
        group: "Electronic & tonal",
        layer: DETAILS,
        about: "Monitors, consoles, alarms, sonar and scanners.",
        params: &[c("Kind", &["Heart monitor", "Console", "Alarm", "Sonar ping", "Scanner", "Crossing bell", "Microwave"], 0.0), e("Rate", 0.05, 10.0, "/s", 0.4), a("Pitch", 0.5), a("Variety", 0.4)],
    },
    Generator {
        id: "energy",
        name: "Energy hum",
        group: "Electronic & tonal",
        layer: BED,
        about: "Power for the imagined: reactors, force fields, portals, engine cores.",
        params: &[c("Kind", &["Reactor", "Force field", "Portal", "Engine core", "Hologram"], 0.0), e("Pitch", 20.0, 400.0, "Hz", 0.4), a("Pulse", 0.3), a("Harmonics", 0.5), a("Warble", 0.3)],
    },
    Generator {
        id: "data",
        name: "Data & glitches",
        group: "Electronic & tonal",
        layer: DETAILS,
        about: "Computers thinking aloud: data chatter, glitches, modems, telemetry.",
        params: &[c("Kind", &["Data stream", "Glitch", "Modem", "Bleeps", "Teletext"], 0.0), e("Rate", 0.1, 20.0, "/s", 0.4), a("Pitch", 0.5), a("Crush", 0.3)],
    },
    Generator {
        id: "drone",
        name: "Drone",
        group: "Electronic & tonal",
        layer: BED,
        about: "A held, slowly shifting chord: tension, space, the uncanny.",
        params: &[e("Pitch", 25.0, 440.0, "Hz", 0.35), c("Chord", &["Unison", "Fifth", "Octaves", "Minor", "Major", "Tritone", "Cluster"], 1.0), a("Detune", 0.3), TONE, a("Motion", 0.4), a("Grit", 0.0)],
    },
    Generator {
        id: "shimmer",
        name: "Shimmer",
        group: "Electronic & tonal",
        layer: DETAILS,
        about: "Sparkles and glints: magic, fairies, crystals, stars.",
        params: &[e("Density", 0.2, 40.0, "/s", 0.4), a("Pitch", 0.6), a("Glide", 0.4), a("Spread", 0.6)],
    },
    Generator {
        id: "bells",
        name: "Bells & chimes",
        group: "Electronic & tonal",
        layer: DETAILS,
        about: "Church bells, wind chimes, singing bowls, ship's bells, sleigh bells.",
        params: &[
            c("Kind", &["Church bell", "Wind chimes", "Bamboo chimes", "Singing bowl", "Ship's bell", "Clock chime", "Sleigh bells", "Cowbells", "Hand bell", "Gong"], 1.0),
            e("Rate", 0.02, 8.0, "/s", 0.4),
            a("Pitch", 0.5),
            a("Variety", 0.5),
            DISTANCE,
        ],
    },
    Generator {
        id: "ticks",
        name: "Ticks & clicks",
        group: "Electronic & tonal",
        layer: DETAILS,
        about: "Clocks, a Geiger counter, relays, a metronome.",
        params: &[c("Kind", &["Clock", "Grandfather clock", "Geiger counter", "Relays", "Metronome", "Turn signal"], 0.0), e("Rate", 0.2, 30.0, "/s", 0.25), a("Pitch", 0.5), TONE],
    },
    // Signals.
    Generator {
        id: "sirens",
        name: "Sirens",
        group: "Signals",
        layer: DETAILS,
        about: "Sirens in the distance: wail, yelp, two-tone, air raid.",
        params: &[c("Kind", &["Wail", "Yelp", "Two-tone", "Air raid"], 0.0), e("Rate", 0.005, 0.5, "/s", 0.3), DISTANCE, a("Pitch", 0.5)],
    },
    Generator {
        id: "horns",
        name: "Horns",
        group: "Signals",
        layer: DETAILS,
        about: "Car and truck horns, ship's horns, foghorns, train horns.",
        params: &[c("Kind", &["Car horn", "Truck horn", "Ship's horn", "Foghorn", "Train horn", "Bicycle bell"], 0.0), e("Rate", 0.005, 1.0, "/s", 0.3), DISTANCE, a("Pitch", 0.5)],
    },
    Generator {
        id: "battle",
        name: "Distant battle",
        group: "Signals",
        layer: DETAILS,
        about: "Gunfire and artillery, from a far front to the next street.",
        params: &[a("Gunfire", 0.5), a("Artillery", 0.3), DISTANCE, a("Intensity", 0.5)],
    },
    // Movement.
    Generator {
        id: "passby",
        name: "Pass-bys",
        group: "Vehicles",
        layer: MOVEMENT,
        about: "Things going past with a Doppler sweep: cars, planes, trains, whooshes.",
        params: &[
            c("Kind", &["Car", "Truck", "Motorbike", "Jet", "Prop plane", "Helicopter", "Train", "Whoosh", "Spacecraft", "Flying traffic", "Bicycle", "Horse cart"], 0.0),
            e("Rate", 0.005, 1.0, "/s", 0.4),
            DISTANCE,
            a("Speed", 0.5),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_generator_is_well_formed() {
        let mut ids = std::collections::HashSet::new();
        for g in GENERATORS {
            assert!(ids.insert(g.id), "{} twice", g.id);
            assert!(g.params.len() <= 6, "{}", g.id);
            assert!(GROUPS.contains(&g.group), "{}", g.id);
            assert!(g.layer < 4);
            for p in g.params {
                match p.kind {
                    Kind::Range { lo, hi, exp, .. } => {
                        assert!((0.0..=1.0).contains(&p.default), "{} {}", g.id, p.name);
                        assert!(hi > lo && (!exp || lo > 0.0), "{} {}", g.id, p.name);
                    }
                    Kind::Choice(o) => assert!((p.default as usize) < o.len(), "{} {}", g.id, p.name),
                }
            }
        }
    }

    #[test]
    fn values_map_and_describe() {
        let p = e("Rate", 1.0, 100.0, "Hz", 0.5);
        assert!((p.value(0.5) - 10.0).abs() < 1e-3);
        assert_eq!(p.describe(1.0), "100 Hz");
        let c = c("Kind", &["A", "B"], 0.0);
        assert_eq!(c.describe(1.0), "B");
        assert_eq!(c.value(7.0), 1.0);
    }
}
