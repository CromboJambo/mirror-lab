use mirror_log::db;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: init_noop_check <db>");
    let before = db::db_info(&db::init_db(&path).expect("init")).expect("info");
    let conn = db::init_db(&path).expect("second init");
    let after = db::db_info(&conn).expect("info");
    let ver = db::current_schema_version(&conn).expect("version");
    println!("before={before:?} after={after:?} user_version={ver}");
    assert_eq!(before, after, "init_db changed the data!");
    println!("NO-OP OK");
}
