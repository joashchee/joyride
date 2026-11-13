use asset_core::import::{apply, plan, Decision, Options};
use asset_core::{Catalog, Filter, Licence};
use std::fs;
use std::path::Path;

fn project(root: &Path) {
    fs::create_dir_all(root.join("img")).unwrap();
    fs::create_dir_all(root.join("sfx")).unwrap();
    fs::create_dir_all(root.join("build")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("img/hero.png"), vec![1u8; 5000]).unwrap();
    fs::write(root.join("img/hero-copy.png"), vec![1u8; 5000]).unwrap(); // identical
    fs::write(root.join("img/tile.png"), vec![2u8; 3000]).unwrap();
    fs::write(root.join("sfx/boom.ogg"), vec![3u8; 4000]).unwrap();
    fs::write(root.join("main.lua"), "print('hi')").unwrap();
    fs::write(root.join("build/skip.png"), vec![9u8; 100]).unwrap();
    fs::write(root.join(".git/skip.png"), vec![8u8; 100]).unwrap();
    fs::write(root.join("empty.png"), b"").unwrap();
    fs::write(root.join("img/LICENSE.txt"), "Creative Commons Zero: CC0 1.0 Universal").unwrap();
}

fn decide(hash: &str, spdx: &str, by: &str) -> Decision {
    Decision {
        hash: hash.into(),
        licence: Licence::from_spdx(spdx),
        confirmed_by: by.into(),
        note: String::new(),
        author: String::new(),
        source_url: String::new(),
        attribution_text: String::new(),
        tags: vec!["Pixel Art".into()],
    }
}

#[test]
fn plan_groups_duplicates_skips_junk_and_copies_nothing() {
    let d = tempfile::tempdir().unwrap();
    let proj = d.path().join("game");
    let lib = d.path().join("lib");
    project(&proj);
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    assert_eq!(p.project, "game");
    assert_eq!(p.assets.len(), 3); // hero(+copy), tile, boom; no lua, build, .git or empty
    let hero = p.assets.iter().find(|a| a.name == "hero-copy.png" || a.name == "hero.png").unwrap();
    assert_eq!(hero.paths.len(), 2);
    assert_eq!(p.duplicate_bytes(), 5000);
    assert_eq!(hero.guesses[0].spdx, "CC0-1.0");
    assert!(p.assets.iter().find(|a| a.name == "boom.ogg").unwrap().guesses.is_empty());
    assert!(!lib.exists(), "planning must not write to the library");
    let with_code = plan(&mut cat, &proj, &Options { include_code: true }).unwrap();
    assert_eq!(with_code.assets.len(), 4);
}

#[test]
fn unconfirmed_content_is_not_imported() {
    let d = tempfile::tempdir().unwrap();
    let (proj, lib) = (d.path().join("game"), d.path().join("lib"));
    project(&proj);
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    let tile = p.assets.iter().find(|a| a.name == "tile.png").unwrap();
    let rep = apply(&mut cat, &lib, &p, &[decide(&tile.hash, "CC0-1.0", "  ")]).unwrap();
    assert!(rep.imported.is_empty() && rep.linked.is_empty());
    assert_eq!(rep.skipped.len(), 3);
    assert!(cat.search("", &Filter::default()).unwrap().is_empty());
    assert!(!lib.exists());
}

#[test]
fn import_stores_once_records_uses_and_leaves_originals() {
    let d = tempfile::tempdir().unwrap();
    let (proj, lib) = (d.path().join("game"), d.path().join("lib"));
    project(&proj);
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    let ds: Vec<Decision> = p.assets.iter().map(|a| decide(&a.hash, "CC0-1.0", "me")).collect();
    let rep = apply(&mut cat, &lib, &p, &ds).unwrap();
    assert_eq!(rep.imported.len(), 3);
    assert_eq!(rep.bytes_saved, 5000);
    assert!(rep.failed.is_empty() && rep.conflicts.is_empty());
    assert!(proj.join("img/hero.png").exists() && proj.join("img/hero-copy.png").exists());
    let hero = p.assets.iter().find(|a| a.paths.len() == 2).unwrap();
    let id = cat.find_by_hash(&hero.hash).unwrap().unwrap();
    let a = cat.get(id).unwrap().unwrap();
    assert!(a.is_usable());
    assert_eq!(a.tags, vec!["pixel-art"]);
    assert_eq!(cat.uses(id).unwrap().len(), 2);
    assert_eq!(fs::read(lib.join(&a.stored)).unwrap().len(), 5000);
    assert_eq!(cat.origins(id).unwrap()[0].0, "project");
    // Searchable by tag and licence.
    assert_eq!(cat.search("pixel", &Filter::default()).unwrap().len(), 3);
}

#[test]
fn importing_a_second_project_dedupes_against_the_library() {
    let d = tempfile::tempdir().unwrap();
    let (one, two, lib) = (d.path().join("one"), d.path().join("two"), d.path().join("lib"));
    project(&one);
    fs::create_dir_all(&two).unwrap();
    fs::write(two.join("sprite.png"), vec![2u8; 3000]).unwrap(); // same bytes as one's tile.png
    fs::write(two.join("new.png"), vec![7u8; 1000]).unwrap();
    let mut cat = Catalog::in_memory().unwrap();
    let p1 = plan(&mut cat, &one, &Options::default()).unwrap();
    let ds: Vec<_> = p1.assets.iter().map(|a| decide(&a.hash, "CC0-1.0", "me")).collect();
    apply(&mut cat, &lib, &p1, &ds).unwrap();

    let p2 = plan(&mut cat, &two, &Options::default()).unwrap();
    let dup = p2.assets.iter().find(|a| a.name == "sprite.png").unwrap();
    assert!(dup.existing.is_some(), "plan shows the library already holds it");
    let ds: Vec<_> = p2.assets.iter().map(|a| decide(&a.hash, "CC0-1.0", "me")).collect();
    let rep = apply(&mut cat, &lib, &p2, &ds).unwrap();
    assert_eq!((rep.imported.len(), rep.linked.len()), (1, 1));
    assert_eq!(rep.bytes_saved, 3000);
    assert_eq!(cat.search("", &Filter::default()).unwrap().len(), 4);
    assert_eq!(cat.uses(dup.existing.unwrap()).unwrap().len(), 2); // one in each project
}

#[test]
fn a_different_confirmed_licence_is_a_conflict_and_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (one, two, lib) = (d.path().join("one"), d.path().join("two"), d.path().join("lib"));
    fs::create_dir_all(&one).unwrap();
    fs::create_dir_all(&two).unwrap();
    fs::write(one.join("a.png"), vec![5u8; 800]).unwrap();
    fs::write(two.join("b.png"), vec![5u8; 800]).unwrap();
    let mut cat = Catalog::in_memory().unwrap();
    let p1 = plan(&mut cat, &one, &Options::default()).unwrap();
    apply(&mut cat, &lib, &p1, &[decide(&p1.assets[0].hash, "CC0-1.0", "me")]).unwrap();
    let p2 = plan(&mut cat, &two, &Options::default()).unwrap();
    let rep = apply(&mut cat, &lib, &p2, &[decide(&p2.assets[0].hash, "CC-BY-NC-4.0", "me")]).unwrap();
    assert_eq!(rep.conflicts.len(), 1);
    assert_eq!(rep.conflicts[0].existing, "CC0-1.0");
    let id = cat.find_by_hash(&p2.assets[0].hash).unwrap().unwrap();
    assert_eq!(cat.get(id).unwrap().unwrap().licence.unwrap().spdx, "CC0-1.0");
    assert_eq!(cat.uses(id).unwrap().len(), 1, "no use was added for the conflicting project");
}

#[test]
fn a_file_changed_after_planning_fails_without_storing_it() {
    let d = tempfile::tempdir().unwrap();
    let (proj, lib) = (d.path().join("p"), d.path().join("lib"));
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join("a.png"), vec![4u8; 500]).unwrap();
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    fs::write(proj.join("a.png"), vec![6u8; 500]).unwrap();
    let rep = apply(&mut cat, &lib, &p, &[decide(&p.assets[0].hash, "CC0-1.0", "me")]).unwrap();
    assert_eq!(rep.failed.len(), 1);
    assert!(cat.search("", &Filter::default()).unwrap().is_empty());
}

#[test]
fn audio_is_probed_measured_and_tagged_from_its_name() {
    use asset_core::Kind;
    let d = tempfile::tempdir().unwrap();
    let (proj, lib) = (d.path().join("p"), d.path().join("lib"));
    fs::create_dir_all(proj.join("music")).unwrap();
    // One second of half-scale 16-bit mono at 8 kHz.
    let samples: Vec<u8> = (0..8000).flat_map(|_| 16384i16.to_le_bytes()).collect();
    let mut w = b"RIFF".to_vec();
    w.extend((36 + samples.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend([1, 0, 1, 0]);
    w.extend(8000u32.to_le_bytes());
    w.extend(16000u32.to_le_bytes());
    w.extend([2, 0, 16, 0]);
    w.extend(b"data");
    w.extend((samples.len() as u32).to_le_bytes());
    w.extend(samples);
    fs::write(proj.join("music/MUS_Forest_Explore_A_96_Dm_v03.wav"), &w).unwrap();
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    assert_eq!(p.assets[0].audio.as_ref().unwrap().duration_ms, 1000);
    apply(&mut cat, &lib, &p, &[decide(&p.assets[0].hash, "CC0-1.0", "me")]).unwrap();
    let found = cat.search("", &Filter { kind: Some(Kind::Audio), bpm: Some((90, 100)), ..Default::default() }).unwrap();
    assert_eq!(found.len(), 1);
    let a = &found[0];
    assert!(a.tags.contains(&"game:area/forest".to_string()) && a.tags.contains(&"key:d-minor".to_string()));
    let (info, lv) = cat.audio(a.id).unwrap().unwrap();
    assert_eq!((info.sample_rate, info.duration_ms), (8000, 1000));
    assert!((lv.unwrap().peak_dbfs + 6.02).abs() < 0.01);
}

#[test]
fn images_and_sounds_get_previews_and_facet_tags_on_import() {
    use asset_core::Kind;
    let d = tempfile::tempdir().unwrap();
    let (proj, lib) = (d.path().join("p"), d.path().join("lib"));
    fs::create_dir_all(proj.join("art")).unwrap();
    image::RgbaImage::from_fn(16, 16, |x, _| if x < 8 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) }).save(proj.join("art/tile.png")).unwrap();
    fs::write(proj.join("art/broken.png"), b"not really a png").unwrap();
    let samples: Vec<u8> = (0..800).flat_map(|i| (if i < 400 { 0i16 } else { 16384 }).to_le_bytes()).collect();
    let mut w = b"RIFF".to_vec();
    w.extend((36 + samples.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend([1, 0, 1, 0]);
    w.extend(8000u32.to_le_bytes());
    w.extend(16000u32.to_le_bytes());
    w.extend([2, 0, 16, 0]);
    w.extend(b"data");
    w.extend((samples.len() as u32).to_le_bytes());
    w.extend(samples);
    fs::write(proj.join("art/beep.wav"), w).unwrap();
    let mut cat = Catalog::in_memory().unwrap();
    let p = plan(&mut cat, &proj, &Options::default()).unwrap();
    let ds: Vec<Decision> = p.assets.iter().map(|a| decide(&a.hash, "CC0-1.0", "me")).collect();
    let rep = apply(&mut cat, &lib, &p, &ds).unwrap();
    assert_eq!(rep.imported.len(), 3, "an undecodable image is still imported");
    let by = |name: &str| cat.search(name, &Filter::default()).unwrap().remove(0);
    let tile = by("tile");
    for t in ["size:16x16", "style:pixel-art", "palette:limited", "size-class:tiny"] {
        assert!(tile.tags.contains(&t.to_string()), "{t} in {:?}", tile.tags);
    }
    let pv = cat.preview(tile.id).unwrap().unwrap();
    assert_eq!(pv.thumb_size, Some((128, 128)));
    assert!(pv.thumb_png.unwrap().starts_with(b"\x89PNG"));
    assert_eq!(cat.preview(by("broken").id).unwrap(), None);
    let beep = by("beep");
    let wave = cat.preview(beep.id).unwrap().unwrap().waveform.unwrap();
    assert_eq!((wave.len(), wave[0], wave[127]), (128, 0, 128));
    // Facets work as filters.
    let hits = cat.search("", &Filter { kind: Some(Kind::Image), tag: Some("style:pixel-art".into()), ..Default::default() }).unwrap();
    assert_eq!(hits.len(), 1);
}
