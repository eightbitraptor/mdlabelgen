use std::error::Error;
use std::fs;

use ab_glyph::{FontRef, PxScale};
use clap::Parser;
use dirs::{self, download_dir};
use imageproc::{drawing, image};
use imageproc::image::{ImageBuffer, Pixel, Rgb, RgbImage};

use serde_derive::Deserialize;

// Printable Zink sheets are 2 x 3 inches (50 x 76mm)
const PRINTABLE_HEIGHT: u32 = 50;
const PRITNABLE_WIDTH: u32 = 76;

// MD labels (on Sony disks) are 53 x 36 mm safely
const LABEL_HEIGHT: u32 = 50;
const LABEL_WIDTH: u32 = 36;

// 600 dpi ~= 24 dpmm
const DESIRED_DPMM: u32 = 24;

const LABEL_WIDTH_PX: u32 = LABEL_WIDTH * DESIRED_DPMM;
const LABEL_HEIGHT_PX: u32 = LABEL_HEIGHT * DESIRED_DPMM;
const PRITNABLE_WIDTH_PX: u32 = PRITNABLE_WIDTH * DESIRED_DPMM;
const PRINTABLE_HEIGHT_PX: u32 = PRINTABLE_HEIGHT * DESIRED_DPMM;

const PADDING: i32 = 40;
const MARGIN: i32 = 20;

const MD_LOGO_SIZE: u32 = 160;
const TEXT_SIZE_PT: f32 = 60.0;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    cover: Option<String>,

    #[arg(short, long)]
    title: Option<String>,

    #[arg(short, long)]
    artist: Option<String>,

    #[arg(short, long)]
    release_year: Option<String>,

    #[arg(short, long)]
    output: String,

    #[arg(short, long)]
    layout: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    labels: Vec<Label>,
}

#[derive(Deserialize, Debug)]
struct Label {
    title: String,
    artist: String,
    release_year: Option<String>,
    cover: String,
}

fn cover_image(path: &str) -> Result<RgbImage, Box<dyn Error>> {
    let mut cover_image = image::open(fs::canonicalize(path)?)
        .unwrap()
        .into_rgb8();

    cover_image = image::imageops::resize(
        &cover_image, LABEL_WIDTH_PX as u32, LABEL_WIDTH_PX as u32,
        image::imageops::FilterType::Triangle
    );

    Ok(cover_image)
}

fn overlay_text(
    label: RgbImage,
    title_text: &String,
    artist_text: &String,
    release_year: &Option<String>
) -> Result<RgbImage, Box<dyn Error>> {
    const TEXT_AREA_HEIGHT: u32 = LABEL_HEIGHT_PX - LABEL_WIDTH_PX;
    const LINE_HEIGHT: u32 = TEXT_AREA_HEIGHT / 3;

    let font_scale = PxScale::from(TEXT_SIZE_PT);

    let font = FontRef::try_from_slice(
        include_bytes!("../res/liberation_sans/LiberationSans-Bold.ttf")
    )?;

    let first_line_y = LABEL_WIDTH_PX as i32 + PADDING;
    let second_line_y = first_line_y + font_scale.y as i32 + PADDING;
    let third_line_y = second_line_y + LINE_HEIGHT as i32 as i32;

    let white = Rgb([255,255,255]);

    let mut final_label = drawing::draw_text(&label, white, PADDING, first_line_y,
        font_scale , &font, &title_text
    );
    final_label = drawing::draw_text(&final_label, white, PADDING, second_line_y,
        font_scale , &font, &artist_text
    );
    final_label = match release_year {
        Some(year) => drawing::draw_text(
            &final_label, white, PADDING, third_line_y,
            font_scale , &font, &year
        ),
        None => final_label
    };
    Ok(final_label)
}

fn overlay_minidisc_logo(image: &mut RgbImage) -> Result<(), Box<dyn Error>> {
    let md_logo_path = download_dir()
        .ok_or("can't get download dir")?
        .as_path().join("md30wiki_color.png");

    let md_logo = image::imageops::resize(
        &image::open(md_logo_path)?.into_rgb8(),
        MD_LOGO_SIZE as u32, MD_LOGO_SIZE as u32,
        image::imageops::FilterType::CatmullRom
    );
    image::imageops::overlay(image, &md_logo,
        (LABEL_WIDTH_PX - (PADDING / 2) as u32 - MD_LOGO_SIZE) as i64,
        (LABEL_HEIGHT_PX - (PADDING / 2) as u32 - MD_LOGO_SIZE) as i64,
    );

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse the main CLI options
    let args = Args::parse();

    let label_config: Config;

    if args.layout.is_some() {
        let toml_string = fs::read_to_string(args.layout.ok_or("")?)?;
        println!("{:?}", toml_string);
        label_config = toml::from_str(&toml_string)?;
    } else {
        let artist_text = args.artist.ok_or("")?.to_uppercase();
        let title_text = args.title.ok_or("")?.to_uppercase();
        let release_year = args.release_year.and_then(|f| Some(f.to_uppercase()));
        let cover_path = args.cover.ok_or("")?;

        label_config = Config {
            labels: vec![
                Label{ title: title_text, artist: artist_text, release_year: release_year, cover: cover_path }
            ]
        };
    }

    // Create a white background image the same size as a Zink printable sticker
    let mut printable_area: RgbImage = ImageBuffer::new(PRITNABLE_WIDTH_PX, PRINTABLE_HEIGHT_PX);
    for (_x, _y, p) in printable_area.enumerate_pixels_mut() {
        p.invert();
    };

    // Iterate the labels and place them on the image
    for (pos, label_info) in label_config.labels.iter().enumerate() {
        // Generate the Label image, with the cover art, overlaid text and minidisc logo
        let mut label: RgbImage = ImageBuffer::new(LABEL_WIDTH_PX, LABEL_HEIGHT_PX);
        image::imageops::overlay(&mut label, &cover_image(&label_info.cover.as_str())?, 0, 0);
        overlay_minidisc_logo(&mut label)?;
        label = overlay_text(label,
            &label_info.title,
            &label_info.artist,
            &label_info.release_year,
        )?;

        // Push each label onto the background
        let x_pos = (pos * LABEL_WIDTH_PX as usize) + (MARGIN as usize * (pos + 2));
        image::imageops::overlay(&mut printable_area, &label, x_pos as i64, 0);
    }

    // Save the final file to disk
    printable_area.save(args.output)?;
    Ok(())
}