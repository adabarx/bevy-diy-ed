#![allow(dead_code)]

use std::{fs, path::PathBuf};

use bevy::{
    prelude::*, 
    a11y::{
        accesskit::{NodeBuilder, Role},
        AccessibilityNode,
    },
    input::mouse::{MouseScrollUnit, MouseWheel},
    winit::WinitSettings,
};
use clap::Parser;
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};

pub struct Child {
    screen_share: f32,
    id: Entity,
}

impl Child {
    pub fn new(id: Entity) -> Self {
        Self { id, screen_share: 1.0 }
    }
}

#[derive(Component, Default)]
pub enum SplitDir {
    #[default]
    Vertical,
    Horizontal
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component, Default)]
pub struct Document(Vec<Entity>);

#[derive(Component, Default)]
pub struct Windows(Vec<Entity>);

#[derive(Component, Default, Deref, DerefMut)]
pub struct ScrollPosition{ value: f32 }

pub struct WindowBundle {
    windows: Windows,
    split_dir: SplitDir,
}

#[derive(Parser, Debug)]
struct CLI { path: Option<PathBuf> }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
        .add_plugins(PerfUiPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .add_systems(Update, mouse_scroll)
        .run();
}

fn setup(mut commands: Commands, _asset_server: Res<AssetServer>) {
    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    commands.spawn((Camera2dBundle::default(), MainCamera));

    commands.spawn(PerfUiCompleteBundle::default());

    let lines: Vec<_> = content.split_inclusive('\n')
        .map(|line| commands.spawn((
            TextBundle {
                text: Text {
                    sections: line.split_inclusive(' ').map(|span| {
                        if span == "\n" {
                            // TODO: figure out why i have to do this in order
                            // to print empty lines correctly
                            TextSection::new(" \n", TextStyle::default())
                        } else {
                            TextSection::new(span, TextStyle::default())
                        }
                    }).collect(),
                    ..Default::default()
                },
                ..Default::default()
            },
            AccessibilityNode(NodeBuilder::new(Role::ListItem)),
        )).id())
        .collect();

    let text_id = commands.spawn((
        Document(lines.clone()),
        NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            ..Default::default()
        },
        ScrollPosition::default(),
        AccessibilityNode(NodeBuilder::new(Role::List)),
    )).id();

    commands.entity(text_id).insert_children(0, &lines);

    let window_id = commands.spawn((
        Windows(vec![text_id]),
        SplitDir::Vertical,
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_self: AlignSelf::Stretch,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                ..Default::default()
            },
            background_color: BackgroundColor::from(Color::rgb(0.1, 0.1, 0.1)),
            ..Default::default()
        },
    )).id();

    commands.entity(window_id).insert_children(0, &[text_id]);
}

fn mouse_scroll(
    mut scrollwheel_evr: EventReader<MouseWheel>,
    mut text_q: Query<(&mut ScrollPosition, &mut Style, &Parent, &Node)>,
    node_q: Query<&Node>,
) {
    for mouse_wheel_event in scrollwheel_evr.read() {
        for (mut scrolling_list, mut style, parent, list_node) in &mut text_q {
            let items_height = list_node.size().y;
            let container_height = node_q.get(parent.get()).unwrap().size().y;

            let max_scroll = (items_height - container_height).max(0.);

            let dy = match mouse_wheel_event.unit {
                MouseScrollUnit::Line => mouse_wheel_event.y * 20.,
                MouseScrollUnit::Pixel => mouse_wheel_event.y,
            };

            scrolling_list.value += dy;
            scrolling_list.value = scrolling_list.value.clamp(-max_scroll, 0.);
            style.top = Val::Px(scrolling_list.value);
        }
    }
}

