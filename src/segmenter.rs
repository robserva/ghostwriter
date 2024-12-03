use opencv::{
    prelude::*,
    core::{Point, Rect, Scalar, Vec4i, Mat, MatTraitConst},
    imgproc::{
        self, CHAIN_APPROX_SIMPLE, RETR_EXTERNAL,
        connected_components, ConnectedComponentsTypes,
    },
    imgcodecs,
    Error as OpenCvError,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Region {
    pub bounds: (i32, i32, i32, i32), // x, y, width, height
    pub center: (i32, i32),
    pub area: f64,
    pub contour_points: Vec<(i32, i32)>,
}

#[derive(Debug, Serialize)]
pub struct SegmentationResult {
    pub regions: Vec<Region>,
    pub image_size: (i32, i32),
}

pub struct ImageAnalyzer {
    min_region_size: f64,
    max_regions: usize,
}

impl ImageAnalyzer {
    pub fn new(min_region_size: f64, max_regions: usize) -> Self {
        Self {
            min_region_size,
            max_regions,
        }
    }

    pub fn analyze_image_file(&self, image_path: &str) -> Result<SegmentationResult, OpenCvError> {
        println!("Reading image from: {}", image_path);

        // Read image
        let image = imgcodecs::imread(image_path, imgcodecs::IMREAD_COLOR)?;
        let (height, width) = (image.rows(), image.cols());
        println!("Image loaded: {}x{}", width, height);

        // Convert to grayscale
        let mut gray = Mat::default();
        imgproc::cvt_color(&image, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;

        // Apply adaptive threshold
        let mut binary = Mat::default();
        imgproc::adaptive_threshold(
            &gray,
            &mut binary,
            255.0,
            imgproc::ADAPTIVE_THRESH_GAUSSIAN_C,
            imgproc::THRESH_BINARY_INV,
            11,
            2.0,
        )?;

        // Find contours
        let mut contours = opencv::types::VectorOfVectorOfPoint::new();

        imgproc::find_contours(
            &binary,
            &mut contours,
            RETR_EXTERNAL as i32,
            CHAIN_APPROX_SIMPLE as i32,
            Point::new(0, 0),
        )?;

        println!("Found {} contours", contours.len());

        // Process regions
        let mut regions = Vec::new();
        let min_area = (width * height) as f64 * self.min_region_size;

        for i in 0..contours.len() {
            let contour = contours.get(i)?;
            let area = imgproc::contour_area(&contour, false)?;

            if area >= min_area {
                let bounds = imgproc::bounding_rect(&contour)?;
                let moments = imgproc::moments(&contour, false)?;

                // Calculate centroid
                let center_x = (moments.m10 / moments.m00) as i32;
                let center_y = (moments.m01 / moments.m00) as i32;

                // Convert contour points to Vec
                let contour_points: Vec<(i32, i32)> = contour.iter()
                    .map(|p| (p.x, p.y))
                    .collect();

                regions.push(Region {
                    bounds: (bounds.x, bounds.y, bounds.width, bounds.height),
                    center: (center_x, center_y),
                    area,
                    contour_points,
                });
            }
        }

        // Sort by area and limit number of regions
        regions.sort_by(|a, b| b.area.partial_cmp(&a.area).unwrap());
        regions.truncate(self.max_regions);

        println!("Processed {} significant regions", regions.len());

        Ok(SegmentationResult {
            regions,
            image_size: (width, height),
        })
    }

    pub fn analyze_with_connected_components(&self, image_path: &str)
        -> Result<Mat, OpenCvError> {
        let image = imgcodecs::imread(image_path, imgcodecs::IMREAD_COLOR)?;
        let mut gray = Mat::default();
        imgproc::cvt_color(&image, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;

        let mut binary = Mat::default();
        imgproc::threshold(&gray, &mut binary, 127.0, 255.0, imgproc::THRESH_BINARY)?;

        // Connected components with stats
        let mut labels = Mat::default();
        let mut stats = Mat::default();
        let mut centroids = Mat::default();

        connected_components(
            &binary,
            &mut labels,
            8,
            opencv::core::CV_32S,
        )?;

        Ok(labels)
    }

    pub fn generate_description(&self, result: &SegmentationResult) -> String {
        let mut description = format!(
            "Image size: {}x{}\nDetected {} regions:\n\n",
            result.image_size.0,
            result.image_size.1,
            result.regions.len()
        );

        for (i, region) in result.regions.iter().enumerate() {
            description.push_str(&format!(
                "Region {}:\n\
                 - Position: ({}, {})\n\
                 - Size: {}x{}\n\
                 - Center: ({}, {})\n\
                 - Area: {:.2} pixels\n\
                 - Relative position: {:.2}%, {:.2}%\n\n",
                i + 1,
                region.bounds.0,
                region.bounds.1,
                region.bounds.2,
                region.bounds.3,
                region.center.0,
                region.center.1,
                region.area,
                (region.center.0 as f64 / result.image_size.0 as f64) * 100.0,
                (region.center.1 as f64 / result.image_size.1 as f64) * 100.0,
            ));
        }

        description
    }
}

pub fn analyze_image(image_path: &str) -> Result<String, OpenCvError> {
    let analyzer = ImageAnalyzer::new(0.01, 10);

    println!("\n=== Contour-based Analysis ===");
    let description = match analyzer.analyze_image_file(image_path) {
        Ok(result) => analyzer.generate_description(&result),
        Err(e) => format!("Error analyzing image with contours: {}", e),
    };

    println!("\n=== Connected Components Analysis ===");
    match analyzer.analyze_with_connected_components(image_path) {
        Ok(labels) => {
            println!("Label matrix size: {}x{}", labels.rows(), labels.cols());

            // Get the label data
            match unsafe { labels.data_typed::<i32>() } {
                Ok(label_data) => {
                    // Print a small section of the label matrix as a sample
                    println!("\nSample of label matrix (top-left 10x10 if available):");
                    let rows = std::cmp::min(10, labels.rows());
                    let cols = std::cmp::min(10, labels.cols());

                    for i in 0..rows {
                        for j in 0..cols {
                            let idx = (i * labels.cols() + j) as usize;
                            print!("{:3} ", label_data[idx]);
                        }
                        println!();
                    }

                    // Count occurrences of each label
                    let mut label_counts = std::collections::HashMap::new();
                    for &label in label_data.iter() {
                        *label_counts.entry(label).or_insert(0) += 1;
                    }

                    println!("\nLabel counts:");
                    for (&label, count) in label_counts.iter() {
                        println!("Label {}: {} pixels", label, count);
                    }
                },
                Err(e) => println!("Error accessing label data: {}", e),
            }
        },
        Err(e) => println!("Error analyzing image with connected components: {}", e),
    }

    println!("\nAnalysis complete");
    Ok(description)
}
