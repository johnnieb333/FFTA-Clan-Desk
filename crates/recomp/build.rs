fn main() {
    #[cfg(windows)] {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "FFTA Clan Desk");
        res.set("FileDescription", "FFTA Clan Desk game runtime");
        res.compile().unwrap();
    }
}
