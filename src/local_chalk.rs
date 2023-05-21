use std::cmp::max;
use std::cmp::min;

use crate::drawing::make_chalk;
use crate::Chalk;
use crate::MainCamera;

use bevy::{
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    window::Window,
};
use bevy_prototype_lyon::prelude::Fill;
use bevy_prototype_lyon::prelude::GeometryBuilder;
use bevy_prototype_lyon::prelude::ShapeBundle;
use bevy_prototype_lyon::shapes;

pub(crate) struct LocalChalkPlugin;

#[derive(Component)]
struct LocalChalkFlag;

#[derive(Component)]
struct LocalCursor;

#[derive(Resource)]
pub(crate) struct LocalChalk(Chalk);

impl LocalChalk {
    pub(crate) fn get(&self) -> &Chalk {
        &self.0
    }
}

impl Default for LocalChalk {
    fn default() -> Self {
        Self(Chalk {
            color: Color::WHITE,
            line_width: 8,
            ..default()
        })
    }
}

impl Plugin for LocalChalkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalChalk>()
            .add_startup_system(startup)
            .add_system(handle_user_input)
            .add_system(update_pressed)
            .add_system(update_chalk)
            .add_system(update_cursor);
    }
}

fn startup(mut commands: Commands, mut chalk: ResMut<LocalChalk>) {
    commands.spawn((
        LocalChalkFlag, //
        make_chalk(chalk.0),
    ));

    let chalk = &mut chalk.0;

    let shape = shapes::Circle {
        radius: 1.0,
        center: Vec2::new(0.0, 0.0),
    };

    commands.spawn((
        LocalCursor,
        ShapeBundle {
            path: GeometryBuilder::build_as(&shape),
            ..default()
        },
        Fill::color(chalk.color),
    ));
}

fn handle_user_input(
    window_q: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut chalk: ResMut<LocalChalk>,
) {
    let (camera, camera_transform) = camera_q.single();
    let chalk = &mut chalk.0;
    let window = window_q.single();

    let prev_chalk = *chalk;

    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate())
    {
        chalk.x = world_position[0] as i32;
        chalk.y = world_position[1] as i32;
    }

    chalk.updated = is_updated(&prev_chalk, chalk);
}

fn update_pressed(
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut chalk: ResMut<LocalChalk>,
) {
    let chalk = &mut chalk.0;
    let was_pressed = chalk.pressed;

    for event in mouse_button_input_events.iter() {
        match event {
            MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            } => {
                chalk.just_released = false;
                chalk.pressed = true;
            }
            MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Released,
            } => {
                chalk.pressed = false;
            }
            _ => {}
        }
    }

    chalk.just_released = was_pressed && !chalk.pressed;
}

fn is_updated(old_chalk: &Chalk, new_chalk: &Chalk) -> bool {
    old_chalk.x != new_chalk.x
        || old_chalk.y != new_chalk.y
        || old_chalk.pressed != new_chalk.pressed
        || old_chalk.just_released != new_chalk.just_released
}

fn update_cursor(
    mut chalk: ResMut<LocalChalk>,
    mut cursor_q: Query<(&mut Fill, &mut Transform), With<LocalCursor>>,
) {
    let chalk = &mut chalk.0;
    let (mut fill, mut transform) = cursor_q.single_mut();

    *fill = Fill::color(chalk.color);

    let scale = chalk.line_width as f32 / 2.0;
    transform.scale = Vec3::new(scale, scale, scale);

    transform.translation = Vec3::new(chalk.x as f32, chalk.y as f32, 0.);
}

fn update_chalk(
    local_chalk: Res<LocalChalk>,
    mut chalk_q: Query<&mut Chalk, With<LocalChalkFlag>>,
) {
    if let Ok(mut chalk) = chalk_q.get_single_mut() {
        *chalk = local_chalk.0;
    }
}

const COLORS: [Color; 7] = [
    Color::WHITE,
    Color::BLUE,
    Color::TURQUOISE,
    Color::GREEN,
    Color::YELLOW,
    Color::ORANGE,
    Color::RED,
];

fn next_color(curr_color: Color) -> Color {
    if let Some(next_color) = COLORS
        .iter()
        .cycle()
        .skip_while(|&&x| x != curr_color)
        .nth(1)
    {
        *next_color
    } else {
        curr_color
    }
}

fn incr_size(size: u32) -> u32 {
    min(100, size * 2)
}

fn decr_size(size: u32) -> u32 {
    max(1, size / 2)
}

impl LocalChalk {
    pub(crate) fn color(&self) -> Color {
        self.0.color
    }
    pub(crate) fn next_color(&mut self) -> Color {
        self.0.color = next_color(self.0.color);
        self.0.color
    }
    pub(crate) fn incr_size(&mut self) -> u32 {
        self.0.line_width = incr_size(self.0.line_width);
        self.0.line_width
    }
    pub(crate) fn decr_size(&mut self) -> u32 {
        self.0.line_width = decr_size(self.0.line_width);
        self.0.line_width
    }
}
