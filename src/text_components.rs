use std::{fs, path::PathBuf};

use bevy::{input::mouse::{MouseScrollUnit, MouseWheel}, prelude::*};

use clap::Parser;

pub struct DocumentPlugin;

impl Plugin for DocumentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, mouse_scroll);
    }
}

#[derive(Parser, Debug)]
struct CLI { path: Option<PathBuf> }

#[derive(Component, Default)]
pub enum SplitDir {
    #[default]
    Vertical,
    Horizontal
}

#[derive(Component, Default, Reflect)]
pub struct Document;

#[derive(Component, Default, Reflect)]
pub struct AppWindow;

#[derive(Component, Default, Deref, DerefMut, Reflect)]
pub struct LineNumber(usize);

#[derive(Component, Reflect)]
pub struct Line;

#[derive(Component, Reflect)]
pub struct Span;

#[derive(Component, Reflect)]
pub struct Character;

#[derive(Component, Default, Deref, DerefMut, Reflect)]
pub struct ScrollPosition(f32);

#[derive(Bundle)]
pub struct WindowsBundle {
    windows: AppWindow,
    split_dir: SplitDir,
    node: NodeBundle,
}

fn setup(mut commands: Commands, _asset_server: Res<AssetServer>) {
    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    commands.spawn(WindowsBundle {
        windows: AppWindow,
        split_dir: SplitDir::Vertical,
        node: NodeBundle {
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
    }).with_children(|parent| {
        parent.spawn((
            Document,
            NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollPosition::default(),
        )).with_children(|parent| {
            for (i, line_str) in content.split('\n').enumerate() {
                parent.spawn((
                    LineNumber(i + 1),
                    Line,
                    NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                )).with_children(|parent| {
                    let mut empty = true;
                    for span_str in line_str.split_inclusive(' ') {
                        empty = false;
                        parent.spawn((
                            Span,
                            NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        )).with_children(|parent| {
                            for ch in span_str.chars() {
                                parent.spawn((
                                    Character,
                                    TextBundle::from_section(ch, Default::default())
                                ));
                            }
                        });
                    }
                    if empty {
                        parent.spawn((
                            Span,
                            NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        )).with_children(|parent| {
                            parent.spawn((
                                Character,
                                TextBundle::from_section(" ", Default::default())
                            ));
                        });
                    }
                });
            }
        });
    });
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

            **scrolling_list += dy;
            **scrolling_list = scrolling_list.clamp(-max_scroll, 0.);
            style.top = Val::Px(**scrolling_list);
        }
    }
}

