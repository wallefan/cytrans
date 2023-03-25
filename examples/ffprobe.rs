use cytube_generator::ffprobe::ffprobe;

fn main() {
    dbg!(ffprobe(std::path::Path::new("test.mkv")));
}
