Example Crab Flow (conceptual)
let frames = vec![
    ">(,∞,,)~<--", // right walking
    ">~(,,∞,)<--", // chomping
    "->~(,,∞)<--", // grabbing
    "->(,∞,,)~<-", // walking back
];

for frame in frames.iter().cycle() {
    scuttle(frame, 1);      // move right
    chomp(frame, "~", 1);   // animate chomping
    print_frame(frame);
    std::thread::sleep(Duration::from_millis(200));
}
