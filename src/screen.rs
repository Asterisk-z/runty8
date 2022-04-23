use crate::app::DevApp;
use crate::editor::SpriteEditor;
use crate::graphics::{whole_screen_vertex_buffer, FRAGMENT_SHADER, VERTEX_SHADER};
use crate::ui::{ElmApp2, Widget};
use crate::{DrawContext, Scene, State};
use glium::glutin::dpi::{LogicalPosition, LogicalSize};
use glium::glutin::event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode};
use glium::glutin::event_loop::ControlFlow;
use glium::uniform;
use glium::uniforms::MagnifySamplerFilter;
use glium::{glutin, Surface};

pub fn do_something<T: ElmApp2 + 'static>(mut draw_context: DrawContext) {
    let mut app = T::init();

    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new().with_inner_size(LogicalSize::new(640.0, 640.0));
    let cb = glutin::ContextBuilder::new();
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();
    {
        display.gl_window().window().set_cursor_visible(false);
    }
    let scale_factor = display.gl_window().window().scale_factor();
    let logical_size = display
        .gl_window()
        .window()
        .inner_size()
        .to_logical(scale_factor);

    let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

    let program =
        glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

    let mut editor = SpriteEditor::init();

    let mut keys = Keys::new();

    let fps = 30_u64;
    let nanoseconds_per_frame = 1_000_000_000 / 60_u64;

    let mut msg_queue = vec![];

    event_loop.run(move |event, _, control_flow| {
        let should_return = handle_event(event, scale_factor, logical_size, control_flow, &mut draw_context.state, &mut keys);

        if let ShouldReturn::Yes = should_return {
            return;
        }

        let next_frame_time = std::time::Instant::now()
        + std::time::Duration::from_nanos(nanoseconds_per_frame);

        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        let mut target = display.draw();
        target.clear_color(1.0, 0.0, 0.0, 1.0);



        {
            draw_context.state.update_keys(&keys);
            match draw_context.state.scene {
                Scene::Editor => {
                    editor.draw(&mut draw_context);
                    editor.update(&mut draw_context.state);
                },
                Scene::App => {
                    let view = app.view();

                    view.draw(&mut draw_context);
                    for msg in &msg_queue {
                        app.update(msg);
                    }
                }
            }
            if draw_context.state.escape.btnp()  {
                draw_context.state.scene.flip();
            }

            keys.reset();
        }

        let image = glium::texture::RawImage2d::from_raw_rgb(draw_context.buffer.to_vec(), (128, 128));
        let texture = glium::texture::SrgbTexture2d::new(&display, image).unwrap();
        let uniforms = uniform! {
            tex: glium::uniforms::Sampler::new(&texture).magnify_filter(MagnifySamplerFilter::Nearest)
        };

        target
            .draw(
                &whole_screen_vertex_buffer(&display),
                &indices,
                &program,
                &uniforms,
                &Default::default(),
            )
            .unwrap();
        target.finish().unwrap();
    });
}

enum ShouldReturn {
    Yes,
    No,
}

fn handle_event(
    event: Event<()>,
    hidpi_factor: f64,
    window_size: LogicalSize<f64>,
    control_flow: &mut ControlFlow,
    state: &mut State,
    keys: &mut Keys,
) -> ShouldReturn {
    match event {
        Event::WindowEvent { event, .. } => match event {
            glutin::event::WindowEvent::CloseRequested => {
                *control_flow = glutin::event_loop::ControlFlow::Exit;

                ShouldReturn::Yes
            }
            // TODO: Handle resize events.
            glutin::event::WindowEvent::CursorMoved { position, .. } => {
                let logical_mouse: LogicalPosition<f64> = position.to_logical(hidpi_factor);

                state.mouse_x = (logical_mouse.x / window_size.width * 128.).floor() as i32;
                state.mouse_y = (logical_mouse.y / window_size.height * 128.).floor() as i32;

                ShouldReturn::Yes
            }
            glutin::event::WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: input_state,
                ..
            } => {
                keys.mouse = Some(input_state == ElementState::Pressed);

                ShouldReturn::Yes
            }
            glutin::event::WindowEvent::KeyboardInput { input, .. } => {
                handle_key(input, keys);
                ShouldReturn::Yes
            }
            _ => ShouldReturn::Yes,
        },
        Event::NewEvents(cause) => match cause {
            glutin::event::StartCause::ResumeTimeReached { .. } => ShouldReturn::No,
            glutin::event::StartCause::Init => ShouldReturn::No,
            _ => ShouldReturn::Yes,
        },
        _ => ShouldReturn::Yes,
    }
}

fn handle_key(input: KeyboardInput, keys: &mut Keys) {
    if let Some(key) = input.virtual_keycode {
        let key_ref = match key {
            VirtualKeyCode::X => &mut keys.x,
            VirtualKeyCode::C => &mut keys.c,
            VirtualKeyCode::Left => &mut keys.left,
            VirtualKeyCode::Up => &mut keys.up,
            VirtualKeyCode::Right => &mut keys.right,
            VirtualKeyCode::Down => &mut keys.down,
            VirtualKeyCode::Escape => &mut keys.escape,

            _ => return,
        };

        *key_ref = Some(input.state == ElementState::Pressed);
    }
}

pub(crate) struct Keys {
    pub(crate) left: Option<bool>,
    pub(crate) right: Option<bool>,
    pub(crate) up: Option<bool>,
    pub(crate) down: Option<bool>,
    pub(crate) x: Option<bool>,
    pub(crate) c: Option<bool>,
    pub(crate) escape: Option<bool>,
    pub(crate) mouse: Option<bool>,
}

impl Keys {
    fn new() -> Self {
        Self {
            left: None,
            right: None,
            up: None,
            down: None,
            x: None,
            c: None,
            escape: None,
            mouse: None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new()
    }
}
