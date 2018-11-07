# weblogviz

A CLI utility to parse apache web logs and show useful statistics

A toy project to "learn me some Rust"

## Usage

You can download a binary of the last release from the [Releases page](https://github.com/punchagan/weblogviz/releases)

`weblogviz` can be run on an individual file, a list of files or a directory. See `weblogviz -h` for more.

If you have a directory containing access logs, you can run the following

```sh
weblogviz /var/log/apache2/access_sitename.log*
```

## LICENSE

GPL v3
