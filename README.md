# RRDPIT

"rrpdit" is a small little tool that can be pointed at a directory on your
system, and produce RPKI RRDP (RFC 8182) notification, snapshot, and
delta files. You will need to use an http server of your preferred
flavour to deliver these files to the world.

## Installing rrdpit

Assuming you have rsync and the C toolchain but not yet [Rust 1.34](#rust) 
or newer, here’s how you get rrdpit installed.

```bash
curl https://sh.rustup.rs -sSf | sh
source ~/.cargo/env
cargo install rrdpit
```

If you have an older version of rrdpit installed, you can update via

```bash
cargo install -f rrdpit
```


## Using rrdpit

You can ask rrdpit for help.

```bash
rrdpit --help
rrdpit 0.0.1
Dist to RPKI RRDP

USAGE:
    rrdpit --https <uri> --rsync <uri> --source <dir> --target <dir> [clean]

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -h, --https <uri>     base rrdp uri
    -r, --rsync <uri>     base rsync uri
    -s, --source <dir>    source directory
    -t, --target <dir>    target directory

ARGS:
    <clean>    Clean up target dir (handle with care!)
```

Note that 'clean' is optional. If used rrdpit will try to clean out the target
dir, i.e. it will remove unused session id dirs, and unused version directories
for delta files which are no longer referenced.

Use this option with extreme prejudice, and never as root.. please.

### Examples

Sync the entire ARIN RPKI repository:
```bash

$ mkdir -p tmp/arin
$ cd tmp/arin/
$ mkdir source
$ rsync -zarvh rsync://rpki.arin.net/repository ./source/
receiving file list ... done
./
arin-rpki-ta.cer
arin-rpki-ta/
arin-rpki-ta/5e4a23ea-e80a-403e-b08c-2171da2157d3.cer
arin-rpki-ta/arin-rpki-ta.crl
arin-rpki-ta/arin-rpki-ta.mft
arin-rpki-ta/5e4a23ea-e80a-403e-b08c-2171da2157d3/
.....
.....

sent 158.78K bytes  received 11.08M bytes  976.85K bytes/sec
total size is 14.25M  speedup is 1.27
```

Now create RRDP files in a target dir:
```bash
$ mkdir target
$ time rrdpit --https https://rpki.arin.net/rrdp/ --rsync rsync://rpki.arin.net/repository/ --source ./source/ --target ./target/

real  0m0.848s
user  0m0.385s
sys   0m0.258s
```

Check that all expected files are there, or well, at least the number:
```bash
$ find ./source/ -type f | wc -l
    7031

$ grep uri ./target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml  | wc -l
    7031
```

(note that that uuid is a randomly generated session id, used when the target dir is empty)

```bash
$ rm source/arin-rpki-ta.cer 
$ time rrdpit --https https://rpki.arin.net/rrdp/ --rsync rsync://rpki.arin.net/repository/ --source ./source/ --target ./target/

real  0m1.484s
user  0m1.285s
sys   0m0.186s

$ find target
target
target/8e142e20-236c-4694-8430-b05693fab150
target/8e142e20-236c-4694-8430-b05693fab150/1
target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2
target/8e142e20-236c-4694-8430-b05693fab150/2/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml
target/notification.xml

$ cat ./target/notification.xml 
<notification xmlns="http://www.ripe.net/rpki/rrdp" version="1" session_id="8e142e20-236c-4694-8430-b05693fab150" serial="2">
  <snapshot uri="https://rpki.arin.net/rrdp/8e142e20-236c-4694-8430-b05693fab150/2/snapshot.xml" hash="3e8645f306c3c9a888c236fc923128959c88ee2d7abad11802a2d201a29f992f" />
  <delta serial="2" uri="https://rpki.arin.net/rrdp/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml" hash="c08298b02f4e53a652bc6bc6d66c136e2ee9181d19d616e90bfd80b50341d6eb" />
</notification>

$ cat ./target/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml 
<delta xmlns="http://www.ripe.net/rpki/rrdp" version="1" session_id="8e142e20-236c-4694-8430-b05693fab150" serial="2">
  <withdraw uri="rsync://rpki.arin.net/repository/arin-rpki-ta.cer" hash="88e8ed8bb7bdafb8942c82cb6816bb65f37372a8f67a26c045c0103e42996b9e" />
</delta>
```

Note that if you sync again, and there are no changes in the source dir, no deltas will be written:

```bash
$ time rrdpit --https https://rpki.arin.net/rrdp/ --rsync rsync://rpki.arin.net/repository/ --source ./source/ --target ./target/

real  0m1.495s
user  0m1.292s
sys   0m0.190s

$ find target
target
target/8e142e20-236c-4694-8430-b05693fab150
target/8e142e20-236c-4694-8430-b05693fab150/1
target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2
target/8e142e20-236c-4694-8430-b05693fab150/2/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml
target/notification.xml
```

## Future

This code can possibly use more testing. And some things can be cleaned up. However, it seems to
work well from the testing we have done.

Of course you can create issues, but given that my main effort is directed at Krill for the 
moment, which includes its own RRDP server, I cannot guarantee that issues will get a high 
priority. Pull requests may get more mileage ;)