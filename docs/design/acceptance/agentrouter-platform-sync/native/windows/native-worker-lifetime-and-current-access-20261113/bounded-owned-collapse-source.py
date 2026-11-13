import ctypes,json,time,re
from pathlib import Path
from datetime import datetime,timezone
mem={s.split(':')[0]:int(s.split(':')[1].split()[0]) for s in Path('/proc/meminfo').read_text().splitlines() if s.startswith(('MemAvailable:','MemFree:'))}
assert mem['MemAvailable']>=512*1024 and mem['MemFree']>=64*1024
header=Path('/tmp/orchester-owned-collapse-kernel-mman-20261113.h').read_text()
assert re.search(r'^#define\s+MADV_COLLAPSE\s+25\b',header,re.M)
c=ctypes.CDLL(None,use_errno=True)
c.mmap.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_long];c.mmap.restype=ctypes.c_void_p
c.madvise.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int];c.madvise.restype=ctypes.c_int
c.munmap.argtypes=[ctypes.c_void_p,ctypes.c_size_t];c.munmap.restype=ctypes.c_int
h=2097152;n=2*h;raw=c.mmap(None,n,3,0x22,-1,0);assert raw!=ctypes.c_void_p(-1).value
a=(raw+h-1)&~(h-1);before=Path('/proc/buddyinfo').read_text().splitlines();huge=0
print(json.dumps({'state':'observing-one-owned-2MiB-collapse','pid':__import__('os').getpid(),'beforeMemoryKiB':mem}),flush=True)
try:
 ctypes.memset(a,0x5a,h)
 ctypes.set_errno(0);start=time.monotonic();result=c.madvise(a,h,25);code=ctypes.get_errno() if result else 0;elapsed=round((time.monotonic()-start)*1000,3)
 inside=False
 for line in Path('/proc/self/smaps').read_text().splitlines():
  if '-' in line.split()[0]:
   first,last=[int(v,16) for v in line.split()[0].split('-')];inside=(first<=a<last or a<=first<a+h)
  elif inside and line.startswith('AnonHugePages:'):huge+=int(line.split()[1])
finally:
 released=c.munmap(raw,n);assert released==0
r={'schemaVersion':1,'recordedAtUtc':datetime.now(timezone.utc).isoformat(),'status':'one-owned-2MiB-collapse-completed','beforeMemoryKiB':mem,'virtualBytes':n,'applicationBytesWritten':h,'collapseRegionBytes':h,'madviseCollapseReturn':result,'errno':code,'elapsedMs':elapsed,'ownRegionAnonHugePagesKiB':huge,'munmapReturn':released,'allOwnedMappingReleased':True,'sysctlOrConfigurationWrites':0,'otherProcessSignalsOrMemoryAccess':0,'buddyBefore':before,'buddyAfter':Path('/proc/buddyinfo').read_text().splitlines(),'windowsRecoveryClaimed':False,'primaryDocumentation':'https://docs.kernel.org/admin-guide/mm/transhuge.html','constantSource':'https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/plain/include/uapi/asm-generic/mman-common.h?h=v6.18'}
Path(__file__).with_name('attempt-safe.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r),flush=True)
