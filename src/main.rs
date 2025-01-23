use apriltag::{families::TagStandard41h12, DetectorBuilder, Family, Image, TagParams};
use apriltag_image::{image::ImageBuffer, ImageExt};
use nokhwa::{pixel_format::LumaFormat, utils::{CameraIndex, RequestedFormat, RequestedFormatType}, Camera};

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Tag distance in meters
    #[clap(long)]
    tag_distance: f64,

    /// The width of the tag in meters
    #[clap(long)]
    tag_width: f64,

    /// The width of a pixel in micrometers
    #[clap(short, long)]
    pixel_width: f64,

    /// The index of the camera to use as it appears to the OS
    #[clap(short, long, default_value = "0")]
    camera_index: u32,

    /// The delay in seconds to wait before capturing the image
    #[clap(short, long, default_value = "0")]
    with_delay: f64,
}

fn main() -> anyhow::Result<()> {
    let Cli {
        tag_distance,
        tag_width,
        mut pixel_width,
        camera_index,
        with_delay,
    } = Cli::parse();

    // Convert pixel width from micrometers to meters
    pixel_width /= 1_000_000.0;

    let mut detector = DetectorBuilder::new()
        .add_family_bits(TagStandard41h12::default(), 1)
        .add_family_bits(Family::Tag36h11(Default::default()), 1)
        .build()?;

    let index = CameraIndex::Index(camera_index); 
    let requested = RequestedFormat::new::<LumaFormat>(RequestedFormatType::AbsoluteHighestResolution);
    let mut camera = Camera::new(index, requested)?;

    let stdin = std::io::stdin();

    // wait for enter
    println!("Press Enter to capture a frame");
    let mut input = String::new();
    stdin.read_line(&mut input)?;

    // wait for delay
    std::thread::sleep(std::time::Duration::from_secs_f64(with_delay));
    println!("Capturing frame");

    camera.open_stream()?;
    let frame = camera.frame()?;
    println!("Captured frame");
    let decoded = frame.decode_image::<LumaFormat>()?;

    decoded.save("test.png")?;

    // Convert to older version of image crate
    let decoded = ImageBuffer::from_vec(decoded.width(), decoded.height(), decoded.into_raw()).unwrap();
    let img = Image::from_image_buffer(&decoded);
    let mut detections = detector.detect(&img);

    if detections.len() > 1 {
        println!("Multiple tags found");
    } else if let Some(detection) = detections.pop() {
        println!("Tag ID: {}", detection.id());
        let mut corners = detection.corners().to_vec();
        corners.push(corners[0]);
        let sum: f64 = corners
            .windows(2)
            .map(|window| {
                let [p1, p2] = window else { unreachable!() };
                ((p2[0] - p1[0]).powi(2) + (p2[1] - p1[1]).powi(2)).sqrt()
            })
            .sum();
        let average_image_side_length = sum / 4.0 * pixel_width;
        let focal_length = average_image_side_length / tag_width * tag_distance;
        let fx = focal_length / pixel_width;
        println!("Estimated Focal length: {:.1}mm or {:.0}px", focal_length * 1000.0, fx);
        let Some(pose) = detection.estimate_tag_pose(&TagParams {
            tagsize: tag_width,
            fx,
            fy: fx,
            cx: img.width() as f64 / 2.0,
            cy: img.height() as f64 / 2.0,
        }) else {
            println!("Failed to estimate pose");
            return Ok(());
        };
        let &[x, y, z] = pose.translation().data() else {
            unreachable!();
        };
        let apparent_distance = (x.powi(2) + y.powi(2) + z.powi(2)).sqrt();
        println!("Apparent distance: {:.2}m", apparent_distance);
        println!("Error: {:.1}%", (apparent_distance - tag_distance).abs() / tag_distance * 100.0);
        
    } else {
        println!("No tags found");
    }
    Ok(())
}
