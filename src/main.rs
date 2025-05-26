use apriltag::{families::TagStandard41h12, DetectorBuilder, Family, Image, TagParams};
use apriltag_image::{image::ImageBuffer, ImageExt};
use nokhwa::{
    pixel_format::LumaFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// First tag's distance in meters
    #[clap(long)]
    tag_distance1: f64,

    /// The width of the first tag in meters
    #[clap(long)]
    tag_width: f64,

    /// Second tag's distance in meters
    #[clap(long)]
    tag_distance2: f64,

    /// The index of the camera to use as it appears to the OS
    #[clap(short, long, default_value = "0")]
    camera_index: u32,

    /// The delay in seconds to wait before capturing the image
    #[clap(short, long, default_value = "0")]
    with_delay: f64,
}

fn main() -> anyhow::Result<()> {
    let Cli {
        tag_distance1,
        tag_width,
        tag_distance2,
        camera_index,
        with_delay,
    } = Cli::parse();

    let mut detector = DetectorBuilder::new()
        .add_family_bits(TagStandard41h12::default(), 1)
        .add_family_bits(Family::Tag36h11(Default::default()), 1)
        .build()?;

    let index = CameraIndex::Index(camera_index);
    let requested =
        RequestedFormat::new::<LumaFormat>(RequestedFormatType::AbsoluteHighestResolution);

    let stdin = std::io::stdin();

    // wait for enter
    println!("Press Enter to capture the first frame");
    let mut input = String::new();
    stdin.read_line(&mut input)?;

    // wait for delay
    std::thread::sleep(std::time::Duration::from_secs_f64(with_delay));
    println!("Capturing frame");

    let mut camera = Camera::new(index.clone(), requested)?;
    camera.open_stream()?;
    let mut frame = camera.frame()?;
    drop(camera);
    println!("Captured frame");
    let decoded1 = frame.decode_image::<LumaFormat>()?;
    decoded1.save("test1.png")?;

    // wait for enter
    println!("Press Enter to capture the second frame");
    stdin.read_line(&mut input)?;

    // wait for delay
    std::thread::sleep(std::time::Duration::from_secs_f64(with_delay));
    println!("Capturing frame");

    let mut camera = Camera::new(index, requested)?;
    camera.open_stream()?;
    frame = camera.frame()?;
    drop(camera);
    println!("Captured frame");
    let decoded2 = frame.decode_image::<LumaFormat>()?;
    decoded2.save("test2.png")?;

    // Convert to older version of image crate
    let decoded1 =
        ImageBuffer::from_vec(decoded1.width(), decoded1.height(), decoded1.into_raw()).unwrap();
    let img1 = Image::from_image_buffer(&decoded1);

    // Convert to older version of image crate
    let decoded2 =
        ImageBuffer::from_vec(decoded2.width(), decoded2.height(), decoded2.into_raw()).unwrap();
    let img2 = Image::from_image_buffer(&decoded2);

    let mut detections1 = detector.detect(&img1);
    let mut detections2 = detector.detect(&img2);
    if detections1.len() > 1 {
        println!("Multiple tags found in first image");
        return Ok(());
    }
    if detections2.len() > 1 {
        println!("Multiple tags found in second image");
        return Ok(());
    }
    if detections1.is_empty() {
        println!("No tags found in first image");
        return Ok(());
    }
    if detections2.is_empty() {
        println!("No tags found in second image");
        return Ok(());
    }
    loop {
        input.clear();
        println!("\nType a guess for focal length px");
        stdin.read_line(&mut input)?;
        let Ok(fx) = input.trim().parse() else {
            eprintln!("Failed to read f64");
            continue;
        };
        let detection1 = detections1.last().unwrap();
        let Some(pose) = detection1.estimate_tag_pose(&TagParams {
            tagsize: tag_width,
            fx,
            fy: fx,
            cx: img1.width() as f64 / 2.0,
            cy: img1.height() as f64 / 2.0,
        }) else {
            println!("Failed to estimate pose");
            continue;
        };
        let &[x, y, z] = pose.translation().data() else {
            unreachable!();
        };
        let mut apparent_distance = (x.powi(2) + y.powi(2) + z.powi(2)).sqrt();
        println!("Apparent distance 1: {:.2}m", apparent_distance);
        println!(
            "Error 1: {:.1}%",
            (apparent_distance - tag_distance1).abs() / tag_distance1 * 100.0
        );

        let detection2 = detections2.last().unwrap();
        let Some(pose) = detection2.estimate_tag_pose(&TagParams {
            tagsize: tag_width,
            fx,
            fy: fx,
            cx: img2.width() as f64 / 2.0,
            cy: img2.height() as f64 / 2.0,
        }) else {
            println!("Failed to estimate pose");
            continue;
        };
        let &[x, y, z] = pose.translation().data() else {
            unreachable!();
        };
        apparent_distance = (x.powi(2) + y.powi(2) + z.powi(2)).sqrt();
        println!("Apparent distance 2: {:.2}m", apparent_distance);
        println!(
            "Error 2: {:.1}%",
            (apparent_distance - tag_distance2).abs() / tag_distance2 * 100.0
        );
    }

    Ok(())
}
