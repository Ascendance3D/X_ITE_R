//! REA batch 08, lane multiview (`sub:browser.multi-viewer-thread`): several `EmbeddedViewer`s on one
//! thread and one wgpu device, like several `<x3d-canvas>` on one page. Each viewer has its own world
//! (geometry, PixelTexture and ImageTexture, Scripts with global state, Inline, TimeSensor animation,
//! ParticleSystems with a Surface emitter and a bounded model, rigid bodies). The tests compare every
//! frame of a viewer that shares the thread with another one against the same viewer alone, at MSAA 0
//! and 4, and check reload, drop, resources and the process/thread globals one by one.
//! Skipped (with a message) when no wgpu adapter exists.

mod common;

use common::gpu::device;
use std::path::PathBuf;
use x3d_runtime::particles;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;
const W: u32 = 96;
const H: u32 = 72;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-multiview-b08-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// --- a 2 x 2 PNG, written by hand (stored deflate block), for ImageTexture ----------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;

    for &b in data {
        c ^= b as u32;

        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }

    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);

    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }

    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let mut c = kind.to_vec();
    c.extend_from_slice(body);
    out.extend_from_slice(&c);
    out.extend_from_slice(&crc32(&c).to_be_bytes());
}

fn png(rgb: [[u8; 3]; 4]) -> Vec<u8> {
    let mut raw = vec![];

    for row in 0..2 {
        raw.push(0);

        for col in 0..2 {
            raw.extend_from_slice(&rgb[row * 2 + col]);
        }
    }

    let mut z = vec![0x78, 0x01, 0x01];
    z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
    z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
    z.extend_from_slice(&raw);
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = vec![];
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

// --- worlds ---------------------------------------------------------------------------------------

/// World A: scripted animation with state in a global JavaScript variable, a PixelTexture, an
/// ImageTexture, an Inline and a TimeSensor. `tag` names the Script's prints, `color` the base color.
fn world_script(d: &std::path::Path, tag: &str, color: &str, step: f64) -> PathBuf {
    world_script_pad(d, tag, color, step, 0)
}

/// [`world_script`] with `pad` more Scripts in front (other node ids, other field handles), so two worlds
/// are not the same table of nodes and script fields.
fn world_script_pad(d: &std::path::Path, tag: &str, color: &str, step: f64, pad: usize) -> PathBuf {
    let padding: String = (0..pad).map(|i| format!("Group {{ children Script {{ field SFFloat a{i} {i} field MFFloat b{i} [1 2 3] eventOut SFString o{i} url \"ecmascript: var pad{i} = {i}; function initialize() {{ o{i} = 'x' + pad{i}; }}\" }} }}\n")).collect();
    std::fs::write(
        d.join("tex.png"),
        png([[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]]),
    )
    .unwrap();
    std::fs::write(d.join("sub.x3dv"), format!("#X3D V3.3 utf8\nPROFILE Full\nTransform {{ translation 0 -1.2 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} }} }} geometry Sphere {{ radius 0.4 }} }} }}\n")).unwrap();
    let p = d.join("main.x3dv");
    std::fs::write(
        &p,
        format!(
            "#X3D V3.3 utf8\nPROFILE Full\n{padding}\
Viewpoint {{ position 0 0 8 }}\n\
DEF T Transform {{ children [\n\
  Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} }} texture ImageTexture {{ url \"tex.png\" repeatS FALSE repeatT FALSE }} }} geometry Box {{ size 1.2 1.2 1.2 }} }}\n\
  Transform {{ translation 1.8 0 0 children Shape {{ appearance Appearance {{ material Material {{}} texture PixelTexture {{ image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }} }} geometry Sphere {{ radius 0.6 }} }} }}\n\
  Inline {{ url \"sub.x3dv\" }} ] }}\n\
DEF TS TimeSensor {{ loop TRUE cycleInterval 1 }}\n\
DEF S Script {{ eventIn SFFloat f eventOut SFVec3f p eventOut SFRotation r\n\
  url \"ecmascript: var count = 0; var hist = []; function f(v) {{ count++; hist.push(count); p = new SFVec3f(Math.sin(count*{step}), Math.cos(count*{step})*0.5, 0); r = new SFRotation(0,1,0,count*{step}); if (count % 10 == 0) print('{tag} ' + count + ' ' + hist.length); }}\" }}\n\
ROUTE TS.fraction_changed TO S.f\nROUTE S.p TO T.set_translation\nROUTE S.r TO T.set_rotation\n"
        ),
    )
    .unwrap();
    p
}

/// Two ParticleSystems: a Surface emitter on a Box with a bounded model and force, a Cone emitter with wind.
const PARTICLES: &str = "ParticleSystem { maxParticles 80 particleLifetime 2 geometryType \"POINT\"
  emitter SurfaceEmitter { speed 0.5 surface Box { size 1 1 1 } }
  physics [ ForcePhysicsModel { force 0 -1 0 } BoundedPhysicsModel { geometry Box { size 4 3 4 } } ]
  appearance Appearance { material Material { emissiveColor 1 1 0 } } }
Transform { translation 2 0 0 children ParticleSystem { maxParticles 60 particleLifetime 1.5 geometryType \"POINT\" emitter ConeEmitter { direction 0 1 0 angle 0.5 speed 1.5 }
  physics WindPhysicsModel { speed 1 gustiness 0.5 direction 1 0 0 } appearance Appearance { material Material { emissiveColor 0 1 1 } } } }";

/// World B: ParticleSystems (Surface emitter on a Box, a bounded model, a Cone emitter, a wind model)
/// and rigid bodies falling on a floor, a Transform following one of them.
fn world_physics(d: &std::path::Path, drop_height: f64) -> PathBuf {
    world_physics_opts(d, drop_height, true, false)
}

/// [`world_physics`]; `particles` false leaves the ParticleSystems out (the X_ITE pictures of oracle/multiview-b08-baseline
/// were recorded without them; particle images are compared in rea_b13_particles.rs); `mesh_floor` makes the floor of the
/// rigid bodies a concave-capable IndexedFaceSet CollidableShape, which builds its collision shape through the
/// process-global geometry provider.
fn world_physics_opts(
    d: &std::path::Path,
    drop_height: f64,
    particles: bool,
    mesh_floor: bool,
) -> PathBuf {
    let p = d.join("main.x3dv");
    let ps = if particles { PARTICLES } else { "" };
    let floor_shape = if mesh_floor {
        "Shape { geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -6 0.5 -2, 6 0.5 -2, 6 0.5 2, -6 0.5 2, -6 -0.5 -2, 6 -0.5 -2, 6 -0.5 2, -6 -0.5 2 ] } coordIndex [ 0 1 2 3 -1 7 6 5 4 -1 0 4 5 1 -1 1 5 6 2 -1 2 6 7 3 -1 3 7 4 0 -1 ] } }"
    } else {
        "Shape { geometry Box { size 12 1 4 } }"
    };
    std::fs::write(
        &p,
        format!(
            "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\nCOMPONENT RigidBodyPhysics:2\n\
Viewpoint {{ position 0 1 9 }}\n\
{ps}\n\
DEF TR Transform {{ children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0 1 0 }} }} geometry Box {{ size 1 1 1 }} }} }}\n\
Transform {{ translation 0 -1.5 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.4 0.4 0.4 }} }} geometry Box {{ size 12 1 4 }} }} }}\n\
DEF RBC RigidBodyCollection {{ gravity 0 -9.8 0\n\
  bodies [ DEF F RigidBody {{ fixed TRUE position 0 -1.5 0 geometry DEF F_C CollidableShape {{ shape {floor_shape} }} }}\n\
           DEF R RigidBody {{ mass 1 position 0 {drop_height} 0 geometry DEF R_C CollidableShape {{ shape Shape {{ geometry Box {{ size 1 1 1 }} }} }} }} ]\n\
  collider DEF CC CollisionCollection {{ collidables [ USE F_C USE R_C ] }} }}\n\
ROUTE R.position TO TR.set_translation\nROUTE R.orientation TO TR.set_rotation\n"
        ),
    )
    .unwrap();
    p
}

// --- harness ----------------------------------------------------------------------------------------

struct Rig {
    device: wgpu::Device,
    queue: wgpu::Queue,
    tex: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Rig {
    fn new(device: wgpu::Device, queue: wgpu::Queue) -> Rig {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("multiview target"),
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        Rig {
            device,
            queue,
            tex,
            view,
        }
    }

    fn viewer(&self, samples: u32) -> EmbeddedViewer {
        let mut v = EmbeddedViewer::new(self.device.clone(), self.queue.clone(), (W, H));
        v.set_multisampling(samples);
        // Mirror the draw so the MSAA sample pattern is the GL oracle's (x3d-shot --gl-samples).
        v.renderer().set_gl_sample_orientation(samples > 0);
        v
    }

    /// Step to `T0 + k * DT`, draw and read the pixels back.
    fn shot(&self, v: &mut EmbeddedViewer, k: u32) -> Vec<u8> {
        v.frame(T0 + k as f64 * DT, &self.view, FORMAT, W, H)
            .unwrap();
        self.read()
    }

    /// [`Rig::shot`] and collect the Browser.print lines the frame moved to the status line.
    fn shotp(&self, v: &mut EmbeddedViewer, k: u32, prints: &mut Vec<String>) -> Vec<u8> {
        let img = self.shot(v, k);
        prints.extend(v.take_prints());

        // A frame moves the prints to the status line: one print per frame here, each one different.
        let status = v.status().to_string();

        if !status.is_empty() && prints.last() != Some(&status) {
            prints.push(status);
        }

        img
    }

    fn read(&self) -> Vec<u8> {
        let bpr = (W * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * H) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(H),
                },
            },
            wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().expect("map");
        let mut out = vec![];

        for y in 0..H {
            out.extend_from_slice(&data[(y * bpr) as usize..(y * bpr + W * 4) as usize]);
        }

        out
    }
}

/// Load, and return what the world printed while it loaded (initialize).
fn load(v: &mut EmbeddedViewer, p: &std::path::Path) -> Vec<String> {
    v.load(
        p,
        LoadOptions {
            root: None,
            start_time: Some(T0),
        },
    )
    .unwrap();
    v.take_prints()
}

/// Pixels that differ by more than 8 levels in any channel.
fn bad_pixels(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len());
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count()
}

fn exact(a: &[u8], b: &[u8]) -> bool {
    a == b
}

fn lit(img: &[u8]) -> usize {
    img.chunks_exact(4)
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
        .count()
}

/// A constant `randomSeed` and constant wind draws: the generators are shared on the thread (like
/// `Math.random` of a page), so a picture comparison needs values that do not depend on the draw order.
fn constant_draws() {
    if std::env::var("MV_RANDOM").is_ok() {
        return particles::use_random_seeds();
    }

    // The oracle page sets Math.random to the constant 0.37: `randomSeed` is `Math.random () * 0xffffffff`.
    particles::set_seed_list(vec![0.37 * 4294967295.0; 200_000]);
    particles::set_force_draws(vec![0.37; 200_000]);
}

const FRAMES: u32 = 40;

/// Every frame of `world` alone.
fn solo_frames(rig: &Rig, samples: u32, world: &std::path::Path) -> (Vec<Vec<u8>>, Vec<String>) {
    constant_draws();
    let mut v = rig.viewer(samples);
    let mut prints = load(&mut v, world);
    let frames = (0..FRAMES)
        .map(|k| rig.shotp(&mut v, k, &mut prints))
        .collect();
    prints.extend(v.take_prints());
    (frames, prints)
}

fn setup() -> Option<Rig> {
    match device() {
        Some((d, q)) => Some(Rig::new(d, q)),
        None => {
            x3d_evidence::no_gpu("no adapter, skipped");
            None
        }
    }
}

// --- tests -------------------------------------------------------------------------------------------

/// Two scripted worlds with global JavaScript state, interleaved frame by frame, equal each viewer alone.
fn scripts_interleaved(samples: u32) {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (
            dir(&format!("s-a-{samples}")),
            dir(&format!("s-b-{samples}")),
        );
        let (a, b) = (
            world_script(&da, "A", "1 0.5 0", 0.31),
            world_script_pad(&db, "B", "0 0.5 1", 0.17, 4),
        );
        let (sa, pa) = solo_frames(&rig, samples, &a);
        let (sb, pb) = solo_frames(&rig, samples, &b);
        assert!(!pa.is_empty() && !pb.is_empty(), "the scripts print");
        assert!(
            lit(&sa[FRAMES as usize - 1]) > 200
                && bad_pixels(&sa[FRAMES as usize - 1], &sb[FRAMES as usize - 1]) > 100,
            "the two worlds differ"
        );

        let (mut va, mut vb) = (rig.viewer(samples), rig.viewer(samples));
        let (mut qa, mut qb) = (load(&mut va, &a), load(&mut vb, &b));

        for k in 0..FRAMES {
            let (fa, fb) = (
                rig.shotp(&mut va, k, &mut qa),
                rig.shotp(&mut vb, k, &mut qb),
            );
            assert!(
                exact(&fa, &sa[k as usize]),
                "A frame {k} equals A alone: {} pixels off",
                bad_pixels(&fa, &sa[k as usize])
            );
            assert!(
                exact(&fb, &sb[k as usize]),
                "B frame {k} equals B alone: {} pixels off",
                bad_pixels(&fb, &sb[k as usize])
            );
        }

        assert_eq!(qa, pa, "A prints");
        assert_eq!(qb, pb, "B prints");
    });
}

#[test]
fn scripts_two_viewers_interleaved_msaa0() {
    scripts_interleaved(0);
}

#[test]
fn scripts_two_viewers_interleaved_msaa4() {
    scripts_interleaved(4);
}

/// Particles (Surface emitter, bounded model, wind) and rigid bodies in two viewers, plus a script world
/// in a third one, interleaved. Draws are constant, see `constant_draws`.
fn physics_interleaved(samples: u32) {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db, dc) = (
            dir(&format!("p-a-{samples}")),
            dir(&format!("p-b-{samples}")),
            dir(&format!("p-c-{samples}")),
        );
        let (a, b, c) = (
            world_physics(&da, 3.0),
            world_physics_opts(&db, 0.5, true, true),
            world_script_pad(&dc, "C", "0.3 1 0.3", 0.23, 7),
        );
        let (sa, _) = solo_frames(&rig, samples, &a);
        let (sb, _) = solo_frames(&rig, samples, &b);
        let (sc, pc) = solo_frames(&rig, samples, &c);
        assert!(
            bad_pixels(&sa[FRAMES as usize - 1], &sb[FRAMES as usize - 1]) > 50,
            "the two physics worlds differ (different drop height)"
        );
        assert!(sa[5] != sa[30], "world A moves");

        constant_draws();
        let (mut va, mut vb, mut vc) = (
            rig.viewer(samples),
            rig.viewer(samples),
            rig.viewer(samples),
        );
        load(&mut va, &a);
        load(&mut vb, &b);
        let mut qc = load(&mut vc, &c);

        for k in 0..FRAMES {
            let (fa, fb, fc) = (
                rig.shot(&mut va, k),
                rig.shot(&mut vb, k),
                rig.shotp(&mut vc, k, &mut qc),
            );
            assert!(
                exact(&fa, &sa[k as usize]),
                "A frame {k}: {} pixels off",
                bad_pixels(&fa, &sa[k as usize])
            );
            assert!(
                exact(&fb, &sb[k as usize]),
                "B frame {k}: {} pixels off",
                bad_pixels(&fb, &sb[k as usize])
            );
            assert!(
                exact(&fc, &sc[k as usize]),
                "C frame {k}: {} pixels off",
                bad_pixels(&fc, &sc[k as usize])
            );
        }

        assert_eq!(qc, pc);
    });
}

#[test]
fn particles_and_physics_three_viewers_msaa0() {
    physics_interleaved(0);
}

#[test]
fn particles_and_physics_three_viewers_msaa4() {
    physics_interleaved(4);
}

/// Reload of one viewer does not disturb the other, which keeps its own Script state; the reloaded one
/// restarts from its initial state, like alone.
#[test]
fn independent_reload() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (dir("r-a"), dir("r-b"));
        let (a, b) = (
            world_script(&da, "A", "1 0.5 0", 0.31),
            world_script_pad(&db, "B", "0 0.5 1", 0.17, 4),
        );
        let (sa, pa) = solo_frames(&rig, 4, &a);
        let (sb, _) = solo_frames(&rig, 4, &b);
        let (mut va, mut vb) = (rig.viewer(4), rig.viewer(4));
        let mut qa = load(&mut va, &a);
        load(&mut vb, &b);

        for k in 0..20 {
            rig.shotp(&mut va, k, &mut qa);
            rig.shot(&mut vb, k);
        }

        // B reloads (a new Script engine) in the middle of A's run.
        vb.reload().unwrap();

        for k in 20..FRAMES {
            let fa = rig.shotp(&mut va, k, &mut qa);
            assert!(
                exact(&fa, &sa[k as usize]),
                "A frame {k} after B reloaded: {} pixels off",
                bad_pixels(&fa, &sa[k as usize])
            );
        }

        for k in 0..FRAMES {
            let fb = rig.shot(&mut vb, k);
            assert!(
                exact(&fb, &sb[k as usize]),
                "reloaded B frame {k}: {} pixels off",
                bad_pixels(&fb, &sb[k as usize])
            );
        }

        assert_eq!(qa, pa);
    });
}

/// Dropping one viewer (its Scripts get shutdown, its engine goes) keeps the other correct, also when
/// the other is the older one and when the dropped one was created last.
#[test]
fn independent_drop() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db, dc) = (dir("d-a"), dir("d-b"), dir("d-c"));
        let (a, b, c) = (
            world_script(&da, "A", "1 0.5 0", 0.31),
            world_script_pad(&db, "B", "0 0.5 1", 0.17, 4),
            world_script_pad(&dc, "C", "0.5 0.5 0.5", 0.11, 9),
        );
        let (sa, pa) = solo_frames(&rig, 4, &a);
        let (sc, pc) = solo_frames(&rig, 4, &c);
        let (mut va, mut vb, mut vc) = (rig.viewer(4), rig.viewer(4), rig.viewer(4));
        let (mut qa, mut qc) = (load(&mut va, &a), load(&mut vc, &c));
        load(&mut vb, &b);

        for k in 0..15 {
            rig.shotp(&mut va, k, &mut qa);
            rig.shot(&mut vb, k);
            rig.shotp(&mut vc, k, &mut qc);
        }

        drop(vb);

        for k in 15..FRAMES {
            let fa = rig.shotp(&mut va, k, &mut qa);
            assert!(
                exact(&fa, &sa[k as usize]),
                "A frame {k} after B was dropped: {} pixels off",
                bad_pixels(&fa, &sa[k as usize])
            );
            let fc = rig.shotp(&mut vc, k, &mut qc);
            assert!(
                exact(&fc, &sc[k as usize]),
                "C frame {k} after B was dropped: {} pixels off",
                bad_pixels(&fc, &sc[k as usize])
            );
        }

        drop(va);

        // C alone again; unloading A (shutdown) did not touch it.
        assert_eq!(qc, pc);
        assert_eq!(qa, pa);
    });
}

/// The unloaded viewer's prints and the survivor's do not mix.
#[test]
fn unload_prints_stay_per_viewer() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let dir_a = dir("u-a");
        let dir_b = dir("u-b");
        let mk = |d: &std::path::Path, tag: &str| {
            let p = d.join("main.x3dv");
            std::fs::write(&p, format!("#X3D V3.3 utf8\nPROFILE Full\nShape {{ geometry Box {{}} }}\nScript {{ url \"ecmascript: function initialize() {{ print('init {tag}'); }} function shutdown() {{ print('shutdown {tag}'); }}\" }}\n")).unwrap();
            p
        };
        let (mut va, mut vb) = (rig.viewer(4), rig.viewer(4));
        let (mut qa, mut qb) = (
            load(&mut va, &mk(&dir_a, "A")),
            load(&mut vb, &mk(&dir_b, "B")),
        );
        rig.shotp(&mut va, 0, &mut qa);
        rig.shotp(&mut vb, 0, &mut qb);
        va.unload();
        qa.extend(va.take_prints());
        assert_eq!(qa, vec!["init A".to_string(), "shutdown A".to_string()]);
        rig.shotp(&mut vb, 1, &mut qb);
        assert_eq!(qb, vec!["init B".to_string()]);
        vb.unload();
        qb.extend(vb.take_prints());
        assert_eq!(qb, vec!["init B".to_string(), "shutdown B".to_string()]);
    });
}

/// Viewers with different rendering options keep their own: a LogarithmicDepthBuffer or quality option
/// of one must not change the other's pictures.
#[test]
fn rendering_options_stay_per_viewer() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (dir("o-a"), dir("o-b"));
        let scene = |opt: &str| {
            format!(
                "#X3D V3.3 utf8\nPROFILE Full\nViewpoint {{ position 0 0 8 }}\nScript {{ url \"ecmascript: function initialize() {{ {opt} }}\" }}\nShape {{ appearance Appearance {{ material Material {{ diffuseColor 1 0 0 }} }} geometry Sphere {{ radius 1.5 }} }}\nTransform {{ translation 0 0 -2 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0 0 1 }} }} geometry Cylinder {{ radius 1 height 4 }} }} }}\n"
            )
        };
        let write = |d: &std::path::Path, text: String| {
            let p = d.join("main.x3dv");
            std::fs::write(&p, text).unwrap();
            p
        };
        let a = write(
            &da,
            scene(
                "Browser.setBrowserOption('PrimitiveQuality', 'LOW'); Browser.setBrowserOption('LogarithmicDepthBuffer', true);",
            ),
        );
        let b = write(&db, scene(""));
        let (sa, _) = solo_frames(&rig, 0, &a);
        let (sb, _) = solo_frames(&rig, 0, &b);
        assert!(
            bad_pixels(&sa[10], &sb[10]) > 20,
            "LOW primitive quality draws a different sphere than MEDIUM"
        );

        let (mut va, mut vb) = (rig.viewer(0), rig.viewer(0));
        load(&mut va, &a);
        load(&mut vb, &b);

        for k in 0..12 {
            let (fa, fb) = (rig.shot(&mut va, k), rig.shot(&mut vb, k));
            assert!(
                exact(&fa, &sa[k as usize]),
                "A (LOW, log depth) frame {k}: {} pixels off",
                bad_pixels(&fa, &sa[k as usize])
            );
            assert!(
                exact(&fb, &sb[k as usize]),
                "B (defaults) frame {k}: {} pixels off",
                bad_pixels(&fb, &sb[k as usize])
            );
        }

        // A screenshot of B right after A stepped uses B's options.
        rig.shot(&mut va, 12);
        let shot_b = vb.screenshot_rgba(W, H).unwrap();
        let shot_b_alone = {
            let mut v = rig.viewer(0);
            load(&mut v, &b);

            for k in 0..12 {
                rig.shot(&mut v, k);
            }

            v.screenshot_rgba(W, H).unwrap()
        };
        assert!(
            exact(&shot_b, &shot_b_alone),
            "B screenshot after A stepped: {} pixels off",
            bad_pixels(&shot_b, &shot_b_alone)
        );
    });
}

/// GPU and runtime resources belong to the viewer: a second viewer's world adds to its own renderer only,
/// and unloading brings the viewer's renderer back to its start. The script engine's handle table is
/// per host.
#[test]
fn resources_released_per_viewer() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (dir("m-a"), dir("m-b"));
        let (a, b) = (
            world_script(&da, "A", "1 0.5 0", 0.31),
            world_physics(&db, 3.0),
        );
        let (mut va, mut vb) = (rig.viewer(4), rig.viewer(4));
        let base = va.renderer().resource_counts();
        assert_eq!(base, vb.renderer().resource_counts());
        load(&mut va, &a);
        rig.shot(&mut va, 0);
        let a_loaded = va.renderer().resource_counts();
        assert!(
            a_loaded[0] > 0 && a_loaded[2] > 0,
            "A uploaded geometry and textures: {a_loaded:?}"
        );
        assert_eq!(
            vb.renderer().resource_counts(),
            base,
            "B has not drawn: nothing of A is in its renderer"
        );
        load(&mut vb, &b);

        for k in 0..10 {
            rig.shot(&mut vb, k);
            rig.shot(&mut va, k);
        }

        let (a_run, b_run) = (
            va.renderer().resource_counts(),
            vb.renderer().resource_counts(),
        );
        let nodes_a = va.core().unwrap().rt.scene.nodes.len();
        let slots_a = x3d_script::engine::slot_stats();
        // Reload B several times: its renderer does not grow, A's resources stay.
        for _ in 0..5 {
            vb.reload().unwrap();

            for k in 0..3 {
                rig.shot(&mut vb, k);
            }
        }

        // The renderer drops what a world stopped using after KEEP_FRAMES (120) unused frames, checked every
        // 30: right after a reload the old world's buffers are still cached (2 per reload alone, too).
        let after_reloads = vb.renderer().resource_counts();
        assert!(
            after_reloads[0] > b_run[0],
            "old buffers are cached for a while: {after_reloads:?}"
        );

        for k in 0..200 {
            rig.shot(&mut vb, k);
        }

        assert_eq!(
            vb.renderer().resource_counts(),
            b_run,
            "B's GPU resources once the old ones aged out"
        );
        assert_eq!(
            va.renderer().resource_counts(),
            a_run,
            "A's GPU resources untouched by B's reloads"
        );
        assert_eq!(va.core().unwrap().rt.scene.nodes.len(), nodes_a);
        let _ = slots_a;

        vb.unload();

        for k in 0..200 {
            rig.shot(&mut vb, k);
        }

        assert_eq!(
            vb.renderer().resource_counts()[0..3],
            [0, 0, 0],
            "B unloaded: geometries, sprites, textures"
        );
        assert_eq!(
            va.renderer().resource_counts(),
            a_run,
            "A untouched by B's unload"
        );
    });
}

/// Script contexts belong to the viewer: each world's host has its own table, dropping or unloading a world
/// releases its host, and the others keep running.
#[test]
fn script_hosts_released_per_viewer() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db, dc) = (dir("h-a"), dir("h-b"), dir("h-c"));
        let (a, b, c) = (
            world_script(&da, "A", "1 0.5 0", 0.31),
            world_script_pad(&db, "B", "0 0.5 1", 0.17, 4),
            world_script_pad(&dc, "C", "0.5 0.5 0.5", 0.11, 9),
        );
        assert_eq!(x3d_script::engine::live_hosts(), 0);
        let (mut va, mut vb, mut vc) = (rig.viewer(0), rig.viewer(0), rig.viewer(0));
        load(&mut va, &a);
        load(&mut vb, &b);
        load(&mut vc, &c);
        assert_eq!(x3d_script::engine::live_hosts(), 3);

        for k in 0..5 {
            rig.shot(&mut va, k);
            rig.shot(&mut vb, k);
            rig.shot(&mut vc, k);
        }

        drop(vb);
        assert_eq!(
            x3d_script::engine::live_hosts(),
            2,
            "dropping a viewer releases its script host"
        );
        vc.unload();
        assert_eq!(
            x3d_script::engine::live_hosts(),
            1,
            "unloading releases the host"
        );
        let table = x3d_script::engine::slot_stats();
        assert!(table.len > 0);

        // 20 reloads of A: one host at a time, none left over.
        for _ in 0..20 {
            va.reload().unwrap();
            rig.shot(&mut va, 0);
            assert_eq!(
                x3d_script::engine::live_hosts(),
                1,
                "a reload replaces the host"
            );
        }

        // Another world on the viewer that was unloaded.
        load(&mut vc, &c);
        assert_eq!(x3d_script::engine::live_hosts(), 2);
        drop(va);
        drop(vc);
        assert_eq!(x3d_script::engine::live_hosts(), 0);
    });
}

/// The seeded random streams are shared on the thread by design (X_ITE: one `Math.random` per page),
/// so viewer B's draws do depend on viewer A's. This documents the behavior with a seed list.
#[test]
fn random_stream_is_shared_on_the_thread() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (dir("g-a"), dir("g-b"));
        let (a, b) = (world_physics(&da, 3.0), world_physics(&db, 3.0));
        particles::set_seed_list((1..=100_000).map(|i| i as f64 * 1234567.0).collect());
        let mut vb = rig.viewer(0);
        load(&mut vb, &b);

        for k in 0..6 {
            rig.shot(&mut vb, k);
        }

        let b_alone = particles::next_seed();

        particles::set_seed_list((1..=100_000).map(|i| i as f64 * 1234567.0).collect());
        let (mut va, mut vb) = (rig.viewer(0), rig.viewer(0));
        load(&mut va, &a);
        load(&mut vb, &b);

        for k in 0..6 {
            rig.shot(&mut va, k);
            rig.shot(&mut vb, k);
        }

        let both = particles::next_seed();
        eprintln!("next seed after 6 frames: B alone {b_alone}, A and B {both}");
        assert_ne!(
            b_alone, both,
            "the seed stream is one per thread: A's passes consume values"
        );
        particles::use_random_seeds();
        particles::use_random_force_draws();
    });
}

// --- the pinned X_ITE: two X3DCanvas on one page vs one each ----------------------------------------------------

fn oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

/// The worlds of the X_ITE comparison, written to `oracle/multiview-b08/worlds/<name>/main.x3dv` (with their
/// textures and Inline). Run by hand when the worlds change, see the lane report:
/// `cargo test -p x3d-viewer --test multi_viewer_b08 -- --ignored gen_oracle_worlds`.
#[test]
#[ignore]
fn gen_oracle_worlds() {
    let root = oracle_dir().join("multiview-b08/worlds");

    for (name, make) in oracle_worlds() {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        make(&d);
    }
}

type Maker = Box<dyn Fn(&std::path::Path)>;

fn oracle_worlds() -> Vec<(&'static str, Maker)> {
    vec![
        (
            "s-a",
            Box::new(|d| drop(world_script(d, "A", "1 0.5 0", 0.31))),
        ),
        (
            "s-b",
            Box::new(|d| drop(world_script_pad(d, "B", "0 0.5 1", 0.17, 4))),
        ),
        (
            "rb-a",
            Box::new(|d| drop(world_physics_opts(d, 3.0, false, false))),
        ),
        (
            "rb-b",
            Box::new(|d| drop(world_physics_opts(d, 0.5, false, true))),
        ),
    ]
}

const ORACLE_FRAMES: [u32; 3] = [5, 20, 40];

fn read_rgba(p: &std::path::Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run oracle/x_ite-multi-b08.mjs, see the report)",
            p.display()
        )
    })
}

/// Rust frames `ORACLE_FRAMES` of the given worlds, all on one thread, interleaved (or one viewer for one world).
fn rust_frames(rig: &Rig, samples: u32, worlds: &[&str]) -> Vec<Vec<Vec<u8>>> {
    constant_draws();
    let paths: Vec<PathBuf> = worlds
        .iter()
        .map(|w| {
            oracle_dir()
                .join("multiview-b08/worlds")
                .join(w)
                .join("main.x3dv")
        })
        .collect();
    let mut viewers: Vec<EmbeddedViewer> = worlds.iter().map(|_| rig.viewer(samples)).collect();

    for (v, p) in viewers.iter_mut().zip(&paths) {
        load(v, p);
        // The oracle page traverses 8 warm-up frames at t0 while the worlds load (x3d-drive does the same).
        let c = v.core().unwrap();
        c.frame_rate = 60.0;
        // The oracle page returns 1 / dt from getCurrentFrameRate (): a RigidBodyCollection's moving-average time step
        // (and so the number of substeps) depends on it. Left to measure the rate from the clock deltas, which are
        // differences of two doubles near 9.5e8, the time step is off by up to 1e-6 relative and the box falls
        // a fraction of a frame ahead (frame 9 on: rb-a 0.58% at MSAA 4). x3d-drive pins it the same way.
        c.pinned_frame_rate = Some(1.0 / DT);

        for _ in 0..std::env::var("MV_WARM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8)
        {
            c.step(T0);
        }
    }

    let mut out = vec![vec![]; worlds.len()];

    if std::env::var("MV_DUMP").is_ok() {
        for (v, w) in viewers.iter_mut().zip(worlds) {
            eprintln!("LOG {w}: {:?}", v.log());
        }
    }

    for k in 1..=*ORACLE_FRAMES.last().unwrap() {
        for (i, v) in viewers.iter_mut().enumerate() {
            let img = rig.shot(v, k);

            if ORACLE_FRAMES.contains(&k) || std::env::var_os("MV_ALL").is_some() {
                if let Ok(d) = std::env::var("MV_DUMP") {
                    std::fs::write(
                        format!("{d}/rust-{}-msaa{samples}-f{k}.rgba", worlds[i]),
                        &img,
                    )
                    .unwrap();
                }

                out[i].push(img);
            }
        }
    }

    out
}

/// Share of the pixels more than 8 levels off in red, green or blue. The alpha of the two canvases differs where
/// nothing is drawn (X_ITE reads back 255, the host's target here keeps 0), which the page shows as black.
fn off(a: &[u8], b: &[u8]) -> f64 {
    let n = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| (0..3).any(|i| p[i].abs_diff(q[i]) > 8))
        .count();
    n as f64 / (a.len() / 4) as f64
}

/// Rust, one viewer per world on one thread, against the pinned X_ITE with one X3DCanvas per world on one page, and
/// both against the same worlds alone. At most 0.5% of the pixels more than 8 levels off.
fn oracle_pair(samples: u32, pair: [&'static str; 2]) {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let base = oracle_dir().join("multiview-b08-baseline");
        let paired = rust_frames(&rig, samples, &pair);
        let mut worst = 0.0f64;
        let mut fails: Vec<String> = vec![];

        for (i, w) in pair.iter().enumerate() {
            let alone = rust_frames(&rig, samples, &[w]).remove(0);

            for (fi, k) in ORACLE_FRAMES.iter().enumerate() {
                let x_pair = read_rgba(&base.join(format!(
                    "{}+{}-msaa{samples}/c{i}-f{k}.rgba",
                    pair[0], pair[1]
                )));
                let x_alone = read_rgba(&base.join(format!("{w}-msaa{samples}/c0-f{k}.rgba")));
                let (a, b, c, e) = (
                    off(&x_pair, &x_alone),
                    off(&paired[i][fi], &x_pair),
                    off(&alone[fi], &x_alone),
                    off(&paired[i][fi], &alone[fi]),
                );
                eprintln!(
                    "MULTIVIEW msaa={samples} world={w} frame={k}: X_ITE pair vs X_ITE alone {a:.5}; Rust pair vs X_ITE pair {b:.5}; Rust alone vs X_ITE alone {c:.5}; Rust pair vs Rust alone {e:.5}"
                );
                worst = worst.max(a).max(b).max(c).max(e);

                // 0.5% everywhere (owner decision 2026-10-18: no wider limit). The rb-a MSAA 4 difference (0.579%) was the
                // harness measuring the frame rate; with it pinned to 1 / dt, like the oracle page, rb-a and rb-b are exact.
                let limit = 0.005;

                if a > 0.005 || b > limit || c > limit || e != 0.0 {
                    fails.push(format!("{w} frame {k}: X_ITE pair/alone {a}, Rust pair/X_ITE pair {b}, Rust alone/X_ITE alone {c}, Rust pair/alone {e}"));
                }
            }
        }

        eprintln!("MULTIVIEW msaa={samples} pair={pair:?} worst {worst:.5}");
        assert!(fails.is_empty(), "{fails:#?}");
    });
}

#[test]
fn x_ite_two_canvases_scripts_msaa0() {
    oracle_pair(0, ["s-a", "s-b"]);
}

#[test]
fn x_ite_two_canvases_scripts_msaa4() {
    oracle_pair(4, ["s-a", "s-b"]);
}

#[test]
fn x_ite_two_canvases_rigid_bodies_msaa0() {
    oracle_pair(0, ["rb-a", "rb-b"]);
}

#[test]
fn x_ite_two_canvases_rigid_bodies_msaa4() {
    oracle_pair(4, ["rb-a", "rb-b"]);
}

// --- particle simulation state ---------------------------------------------------------------------------------

/// The simulation state of every ParticleSystem of a viewer: the slots in use and their float bits. (The renderer draws
/// the particles from this state, so comparing it per frame compares what each viewer's particles are.)
fn particle_state(v: &mut EmbeddedViewer) -> Vec<(usize, Vec<u32>)> {
    let rt = &v.core().unwrap().rt;
    let mut out = vec![];

    for node in 0..rt.nrt.len() as x3d_fields::NodeId {
        if let Some(sim) = rt.particle_sim(node) {
            let bits = sim.particles[..sim.num_particles]
                .iter()
                .flat_map(|p| p.iter().map(|f| f.to_bits()))
                .collect();
            out.push((sim.num_particles, bits));
        }
    }

    out
}

/// Two viewers with ParticleSystems (Surface and Cone emitters, Force, Wind and Bounded models; the Surface emitter
/// and the BoundedPhysicsModel sample geometry through the process-global provider) run interleaved on one thread
/// with a seed list that does not depend on the draw order: each viewer's particles equal the viewer alone, frame
/// by frame, and the particles are really there (the test would otherwise compare empty buffers).
#[test]
fn particle_state_two_viewers_equal_alone() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        let (da, db) = (dir("ps-a"), dir("ps-b"));
        let a = world_physics_opts(&da, 3.0, true, false);
        let b = {
            // Another surface and other counts: the provider must hand each viewer its own geometry.
            let p = db.join("main.x3dv");
            std::fs::write(&p, "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\nParticleSystem { maxParticles 50 particleLifetime 1 geometryType \"POINT\" emitter SurfaceEmitter { speed 1 surface Sphere { radius 2 } }\n  physics [ BoundedPhysicsModel { geometry Box { size 3 3 3 } } ] }\n").unwrap();
            p
        };
        let solo = |w: &std::path::Path| {
            constant_draws();
            let mut v = rig.viewer(0);
            load(&mut v, w);
            let mut states = vec![];

            for k in 0..FRAMES {
                rig.shot(&mut v, k);
                states.push(particle_state(&mut v));
            }

            states
        };
        let (sa, sb) = (solo(&a), solo(&b));
        let last = FRAMES as usize - 1;
        assert!(
            sa[last].len() == 2 && sa[last].iter().all(|(n, _)| *n > 0),
            "A has live particles in both systems: {:?}",
            sa[last].iter().map(|s| s.0).collect::<Vec<_>>()
        );
        assert!(
            sb[last].len() == 1 && sb[last][0].0 > 0,
            "B has live particles"
        );
        assert_ne!(sa[last][0].1, sb[last][0].1);

        constant_draws();
        let (mut va, mut vb) = (rig.viewer(0), rig.viewer(0));
        load(&mut va, &a);
        load(&mut vb, &b);

        for k in 0..FRAMES {
            rig.shot(&mut va, k);
            rig.shot(&mut vb, k);
            assert!(
                particle_state(&mut va) == sa[k as usize],
                "A particles at frame {k}"
            );
            assert!(
                particle_state(&mut vb) == sb[k as usize],
                "B particles at frame {k}"
            );
        }
    });
}

/// The geometry provider is one process-global function pointer; every viewer installs the same one and it is
/// stateless (it reads the Runtime it is handed), so a second viewer changes nothing for the first.
#[test]
fn geometry_provider_is_one_stateless_function() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let rig = Rig::new(d, q);
        // No assertion on the provider before the first load: it is process-global, so whether
        // it is already installed depends on which other test in this binary ran first. What
        // this test owns is that the first load installs one and the second load does not
        // replace it, which the two assertions below carry.
        let (da, db) = (dir("gp-a"), dir("gp-b"));
        let mut va = rig.viewer(0);
        load(&mut va, &world_physics(&da, 3.0));
        let first = x3d_runtime::nodes::particle_systems::geometry_provider()
            .expect("installed by the first viewer");
        let mut vb = rig.viewer(0);
        load(&mut vb, &world_physics_opts(&db, 0.5, true, true));
        let second = x3d_runtime::nodes::particle_systems::geometry_provider().unwrap();
        assert_eq!(
            first as usize, second as usize,
            "the second viewer's install is ignored; both installed the same function"
        );
        assert_eq!(
            first as usize,
            x3d_render::traverse::particle_geometry as usize
        );
    });
}
