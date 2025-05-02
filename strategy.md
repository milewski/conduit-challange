image_a std::image::loader {
    src <- read_file { source <- "./image_a.jpg" }
}

resizer_a std::image::resize {
    input <- image_a::output
    width <- 512
    height <- 512
    output -> write_file { destination <- "./resized-512x512.jpg" }
}

resizer_b from resizer_a {
    width <- 128
    output -> write_file { destination <- "./resized-128x512.jpg" }
}
