#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::time::Duration;
use futures::{select, FutureExt};
use futures_timer::Delay;
use lavagna_core::doc::MutSketch;
use lavagna_core::App;
use lavagna_core::doc::OwnedSketch;
use log::error;
use matchbox_socket::WebRtcSocket;
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::PhysicalSize;
use winit::event::{TouchPhase, WindowEvent, Event, VirtualKeyCode, ElementState, KeyboardInput, MouseButton};
use winit::window::{CursorIcon, Window, WindowBuilder};
use winit::event_loop::{ControlFlow, EventLoop};

pub use pixels::Error;

pub fn run() -> Result<(), Error> {
    let event_loop = EventLoop::new();
    let mut canvas_size = PhysicalSize::new(640, 480);

    let window = {
        WindowBuilder::new()
            .with_title("lavagna")
            .with_inner_size(canvas_size)
            .with_min_inner_size(canvas_size)
            .build(&event_loop)
            .unwrap()
    };

    window.set_cursor_icon(CursorIcon::Crosshair);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.spawn(async {
        let (mut socket, loop_fut) = WebRtcSocket::new("ws://localhost:3536/example_room");

        let loop_fut = loop_fut.fuse();
        futures::pin_mut!(loop_fut);

        let timeout = Delay::new(Duration::from_millis(100));
        futures::pin_mut!(timeout);

        loop {
            for peer in socket.accept_new_connections() {
                let packet = "hello friend!".as_bytes().to_vec().into_boxed_slice();
                socket.send(packet, peer);
            }

            for (peer, packet) in socket.receive() {
                log::info!("Received from {:?}: {:?}", peer, packet);
            }

            select! {
            _ = (&mut timeout).fuse() => {
                timeout.reset(Duration::from_millis(100));
            }

            _ = &mut loop_fut => {
                break;
            }
        }
        }
    });

    let mut app = App::new();
    let mut frozen_sketch: Option<OwnedSketch> = None;
    let mut pixels: Option<Pixels> = None;

    event_loop.run(move |event, _, control_flow| {
        match event {
            // Resumed on Android
            Event::Resumed => {
                canvas_size = window.inner_size();
                pixels = resume(&window, canvas_size, frozen_sketch.take());

                // Prevent drawing a line from the last location when resuming
                app.set_pressed(false);
            }
            // Suspended on Android
            Event::Suspended => {
                frozen_sketch = sketch_from_pixels(pixels.take(), canvas_size);
            }
            // Window resized on Desktop (Linux/Windows/iOS)
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                if let Some(pixels) = pixels.as_mut() {
                    if canvas_size != new_size {
                        resize_buffer(pixels, canvas_size, new_size);
                    }
                } else {
                    frozen_sketch = sketch_from_pixels(pixels.take(), canvas_size);
                    pixels = resume(&window, new_size, frozen_sketch.take());
                    window.request_redraw();
                }

                canvas_size = new_size;
            }
            _ => (),
        }

        if let Some(pixels) = pixels.as_mut() {
            match event {
                Event::RedrawRequested(_) => {
                    let sketch =
                        MutSketch::new(pixels.get_frame(), canvas_size.width, canvas_size.height);
                    app.update(sketch);

                    if pixels
                        .render()
                        .map_err(|e| error!("pixels.render() failed: {}", e))
                        .is_err()
                    {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    if let Ok((x, y)) = pixels.window_pos_to_pixel(position.into()) {
                        app.set_cursor_position(x as isize, y as isize);
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Touch(touch),
                    ..
                } => {
                    match touch.phase {
                        TouchPhase::Started => {
                            app.set_pressed(true);
                        }
                        TouchPhase::Ended => {
                            app.set_pressed(false);
                        }
                        _ => (),
                    }

                    if let Ok((x, y)) = pixels.window_pos_to_pixel(touch.location.into()) {
                        app.set_cursor_position(x as isize, y as isize);
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => {
                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => app.set_pressed(true),
                        (MouseButton::Left, ElementState::Released) => app.set_pressed(false),
                        _ => (),
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { input, .. },
                    ..
                } => {
                    match input {
                        KeyboardInput { state: ElementState::Released, virtual_keycode: Some(VirtualKeyCode::Escape), .. } => { *control_flow = ControlFlow::Exit; }
                        KeyboardInput { state: ElementState::Released, virtual_keycode: Some(VirtualKeyCode::X), .. } => { app.clear_all(); }
                        KeyboardInput { state: ElementState::Released, virtual_keycode: Some(VirtualKeyCode::C), .. } => { app.change_color(); }
                        KeyboardInput { state: ElementState::Released, virtual_keycode: Some(VirtualKeyCode::Z), .. } => { app.resume(); }
                        KeyboardInput { state: ElementState::Released, virtual_keycode: Some(VirtualKeyCode::S), .. } => { app.take_snapshot(); }
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        if app.needs_update() {
            window.request_redraw();
        }
    });
}

fn resume(
    window: &Window,
    new_size: PhysicalSize<u32>,
    frozen_sketch: Option<OwnedSketch>,
) -> Option<Pixels> {
    let surface_texture = SurfaceTexture::new(new_size.width, new_size.height, &window);
    let mut pixels = Pixels::new(new_size.width, new_size.height, surface_texture).ok()?;

    pixels.get_frame().fill(0x00);

    let mut new_sketch = MutSketch::new(pixels.get_frame(), new_size.width, new_size.height);

    if let Some(old_sketch) = &frozen_sketch {
        new_sketch.copy_from(&old_sketch.as_sketch());
    }

    Some(pixels)
}

fn sketch_from_pixels(pixels: Option<Pixels>, canvas_size: PhysicalSize<u32>) -> Option<OwnedSketch> {
    let mut pixels = pixels?;
    let frame = pixels.get_frame();
    let sketch =
        MutSketch::new(frame, canvas_size.width, canvas_size.height);
    Some(sketch.to_owned())
}

fn resize_buffer(pixels: &mut Pixels, canvas_size: PhysicalSize<u32>, new_size: PhysicalSize<u32>) {
    let old_sketch =
        MutSketch::new(pixels.get_frame(), canvas_size.width, canvas_size.height).to_owned();

    pixels.get_frame().fill(0x00);
    pixels.resize_surface(new_size.width, new_size.height);
    pixels.resize_buffer(new_size.width, new_size.height);

    let mut new_sketch = MutSketch::new(pixels.get_frame(), new_size.width, new_size.height);

    new_sketch.copy_from(&old_sketch.as_sketch());
}