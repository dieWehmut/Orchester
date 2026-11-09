# Executed as a fixed script via python3 -c; the database path is an argv value.
import os, sqlite3, sys, tempfile, time, urllib.parse
path = os.path.realpath(sys.argv[1])
limit = 64 * 1024 * 1024
source = sqlite3.connect('file:' + urllib.parse.quote(path) + '?mode=ro', uri=True, timeout=0.2)
source.execute('PRAGMA query_only=ON')
if source.execute('PRAGMA page_count').fetchone()[0] * source.execute('PRAGMA page_size').fetchone()[0] > limit:
    sys.exit(2)
start = time.monotonic()
def progress(status, remaining, total):
    if time.monotonic() - start > 2 or total * source.execute('PRAGMA page_size').fetchone()[0] > limit:
        raise TimeoutError('snapshot limit')
fd, target = tempfile.mkstemp(prefix='orchester-activity-', suffix='.sqlite')
os.close(fd)
try:
    destination = sqlite3.connect(target)
    try:
        source.backup(destination, pages=256, progress=progress, sleep=0.01)
    finally:
        destination.close()
        source.close()
    with open(target, 'rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        sys.exit(2)
    sys.stdout.buffer.write(data)
finally:
    os.unlink(target)
