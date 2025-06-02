# RRDPIT

"rrdpit" is a small little tool that can be pointed at a directory on your
system, and produce RPKI RRDP (RFC 8182) notification, snapshot, and
delta files. You will need to use an http server of your preferred
flavour to deliver these files to the world.

WARNING:
========

This tool is designed to be run *in between* publication runs, but not
during. As part of the syncing process the source directory is crawled
recursively. If there are any changes made to the paths during the sync
then this can result in RRDP snapshots and delta containing inconsistent
repository state.

This situation would resolve itself when rrdpit runs again while there
are no changes being made, but it could cause errors and noise for RPKI
validators.

So, the best option is to run this when it is known that there will be
no changes made to the source. E.g. when a non RRDP capable publication
server knows that it's done writing content, it could trigger rrdpit,
and wait with writing new content until rrdpit is done.

Of course, the safest option might still be to use an RRDP capable RPKI
Publication Server instead so this extra helper tool would not be needed.

## Changelog

### Release 0.1.0

Updated all dependencies to their most recent version. 

The command line interface was rewritten. Some arguments may no longer work 
the same way as they did (most notably the shorthands such as -h).

### Release 0.0.4

Updated _ring_ to 0.17.

### Release 0.0.3

Add option to limit the maximum number of deltas using --max_deltas.
Keeping too many deltas will result in large RRDP notification files if
the individual deltas are much smaller than the snapshot. This can have
a big impact on the server if many RPs request a large notification file.

The default limit is set to 25. This value will work well if rrdpit runs
every minute as it's more than twice the number of the typical RP fetch
interval (10 minutes). If rrdpit runs less frequently then this number
can be lowered. Essentially, one should keep enough deltas so that returning
RPs never need to load the snapshot.

The minimum value of this setting is 1.

### Release 0.0.2

Ignore hidden files in the source directory. I.e. exclude any and all files
and folders starting with a '.' character.

### Release 0.0.1

Initial release.


## Installing rrdpit

Assuming you have rsync and the C toolchain but not yet Rust, here’s how
you get rust installed.

```bash
curl https://sh.rustup.rs -sSf | sh
source ~/.cargo/env
```

If you have an old version of rust installed you may have to update it.
```bash
rustup update
```

To install 'rrdpit' under your user's home directory, use this:

```bash
git clone git@github.com:NLnetLabs/rrdpit.git
cd rrdpit
cargo install
```

If you have an older version of rrdpit installed, you can update via

```bash
cargo install -f
```

## Using rrdpit

You can ask rrdpit for help.

```bash
rrdpit --help
Dist to RPKI RRDP

Usage: rrdpit [OPTIONS] --source <dir> --target <dir> --rsync <uri> --https <uri> [clean]

Arguments:
  [clean]  Clean up target dir (handle with care!)

Options:
      --source <dir>         source directory
      --target <dir>         target directory
      --rsync <uri>          base rsync uri
      --https <uri>          base rrdp uri
      --max_deltas <number>  Limit the maximum number of deltas kept. Default: 25. Minimum: 1
  -h, --help                 Print help
  -V, --version              Print version
```

Note that 'clean' is optional. If used rrdpit will try to clean out the target
dir, i.e. it will remove unused session id dirs, and unused version directories
for delta files which are no longer referenced.

Use this option with care. You do NOT want to use this and accidentally use a
system directory for the `--target` option. Especially if you run this as root,
which would be ill-advised as well.

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
$ time rrdpit --https https://rpki.arin.net/rrdp/ \
              --rsync rsync://rpki.arin.net/repository/ \
              --source ./source/ \
              --target ./target/

real  0m0.848s
user  0m0.385s
sys   0m0.258s
```

Check that all expected files are there, or well, at least the number:
```bash
$ find ./source/ -type f | wc -l
    7031

$ grep uri ./target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml | wc -l
    7031
```

(note that that uuid is a randomly generated session id, used when the target dir is empty)

```bash
$ rm source/arin-rpki-ta.cer 
$ time rrdpit --https https://rpki.arin.net/rrdp/ \
              --rsync rsync://rpki.arin.net/repository/ \
              --source ./source/ \
              --target ./target/

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
$ time rrdpit --https https://rpki.arin.net/rrdp/ \
              --rsync rsync://rpki.arin.net/repository/ \
              --source ./source/ \
              --target ./target/

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

rrdpit will also perform some sanity checks on the existing RRDP files, and if it finds an issue it will use a new session:

```bash
$ find target -type f
target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml
target/notification.xml

$ echo "corrupt" > target//8e142e20-236c-4694-8430-b05693fab150/2/delta.xml


$ rrdpit --https https://rpki.arin.net/rrdp/ \
         --rsync rsync://rpki.arin.net/repository/ \
         --source ./source/ \
         --target ./target/
         
$ find target -type f
target/07cfc1ce-e7d9-4bec-8a70-9feb76778700/1/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/1/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/snapshot.xml
target/8e142e20-236c-4694-8430-b05693fab150/2/delta.xml
target/notification.xml
```

Optionally you can let rrdpit clean up old files as well:
```bash
$ rrdpit --https https://rpki.arin.net/rrdp/ \
         --rsync rsync://rpki.arin.net/repository/ \
         --source ./source/ \
         --target ./target/ \
         clean

$ find target -type f
target/07cfc1ce-e7d9-4bec-8a70-9feb76778700/1/snapshot.xml
target/notification.xml
```



## Future

This code can possibly use more testing. And some things can be cleaned up. However, it seems to
work well from the testing we have done.

Of course you can create issues, but given that our main effort is directed at Krill for the 
moment, which includes its own RRDP server, we cannot guarantee that issues will get a high 
priority. Pull requests may get more mileage ;)
