using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
namespace Orchester.FinalDesktop {
 public static class NativeTray {
  delegate bool Callback(IntPtr hwnd,IntPtr unused);
  [StructLayout(LayoutKind.Sequential)] struct FileTime {public uint Low,High;}
  [DllImport("kernel32.dll")] static extern uint GetProcessId(IntPtr process);
  [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr process,uint milliseconds);
  [DllImport("kernel32.dll")] static extern bool GetProcessTimes(IntPtr process,out FileTime created,out FileTime exited,out FileTime kernel,out FileTime user);
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool QueryFullProcessImageName(IntPtr process,uint flags,StringBuilder image,ref uint size);
  [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback,IntPtr unused);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint process);
  [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder name,int size);
  [DllImport("user32.dll",SetLastError=true)] static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr wparam,IntPtr lparam);
  [StructLayout(LayoutKind.Sequential)] struct IconIdentifier {public uint Size;public IntPtr Window;public uint Id;public Guid Guid;}
  [StructLayout(LayoutKind.Sequential)] struct IconRect {public int Left,Top,Right,Bottom;}
  [DllImport("shell32.dll")] static extern int Shell_NotifyIconGetRect(ref IconIdentifier identifier,out IconRect rect);
  [StructLayout(LayoutKind.Sequential)] struct CursorPoint {public int X,Y;}
  [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern bool GetUserObjectInformation(IntPtr handle,int index,StringBuilder value,uint size,out uint needed);
  [DllImport("user32.dll")] static extern bool GetCursorPos(out CursorPoint point);
  public static int LastPostError {get;private set;}
  static string DesktopName(uint thread){var desktop=GetThreadDesktop(thread);var value=new StringBuilder(256);uint needed;return desktop!=IntPtr.Zero&&GetUserObjectInformation(desktop,2,value,512,out needed)?value.ToString():null;}
  public static object[] ThreadDiagnostics(Process process,uint pid,long created,string image,string sha256,string className){
   var windows=Windows(process,pid,created,image,sha256,className,false);var result=new List<object>();uint harness=GetCurrentThreadId();var harnessName=DesktopName(harness);CursorPoint cursor;bool cursorRead=GetCursorPos(out cursor);
   foreach(var window in windows){RequireWindow(process,pid,created,image,sha256,window,className,false);uint owner;uint thread=GetWindowThreadProcessId(window,out owner);var name=DesktopName(thread);result.Add(new {Window=window.ToInt64(),ThreadId=thread,HarnessThreadId=harness,DesktopNamesReadable=(name!=null&&harnessName!=null),DesktopNamesMatch=(name!=null&&harnessName!=null&&name==harnessName),TrayDesktopIsDefault=(name=="Default"),HarnessDesktopIsDefault=(harnessName=="Default"),CursorRead=cursorRead,CursorX=(cursorRead?cursor.X:0),CursorY=(cursorRead?cursor.Y:0)});}
   Require(process,pid,created,image,sha256);return result.ToArray();
  }
  static string Hash(string path){using(var f=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.Read))using(var sha=SHA256.Create())return BitConverter.ToString(sha.ComputeHash(f)).Replace("-","").ToLowerInvariant();}
  static void Require(Process process,uint pid,long created,string image,string sha256){
   var handle=process.Handle;var path=new StringBuilder(32768);uint size=32768;FileTime c,e,k,u;
   if(GetProcessId(handle)!=pid||WaitForSingleObject(handle,0)!=258||!GetProcessTimes(handle,out c,out e,out k,out u)||!QueryFullProcessImageName(handle,0,path,ref size))throw new InvalidOperationException("tray-process-unavailable");
   if((((long)c.High<<32)|c.Low)!=created||!String.Equals(path.ToString(),image,StringComparison.OrdinalIgnoreCase)||!String.Equals(Hash(path.ToString()),sha256,StringComparison.Ordinal))throw new InvalidOperationException("tray-process-identity-mismatch");
  }
  public static void RequireWindow(Process process,uint pid,long created,string image,string sha256,IntPtr hwnd,string expectedClass,bool visible){
   Require(process,pid,created,image,sha256);uint owner;var name=new StringBuilder(256);
   if(!IsWindow(hwnd)||GetWindowThreadProcessId(hwnd,out owner)==0||owner!=pid||GetClassName(hwnd,name,name.Capacity)==0||name.ToString()!=expectedClass||(visible&&!IsWindowVisible(hwnd)))throw new InvalidOperationException("tray-window-owner-mismatch");
  }
  public static IntPtr[] Windows(Process process,uint pid,long created,string image,string sha256,string expectedClass,bool visible){
   Require(process,pid,created,image,sha256);var result=new List<IntPtr>();
   if(!EnumWindows((hwnd,unused)=>{uint owner;GetWindowThreadProcessId(hwnd,out owner);if(owner!=pid)return true;var name=new StringBuilder(256);if(GetClassName(hwnd,name,name.Capacity)>0&&name.ToString()==expectedClass&&(!visible||IsWindowVisible(hwnd)))result.Add(hwnd);return true;},IntPtr.Zero))throw new InvalidOperationException("tray-window-enumeration-failed");
   Require(process,pid,created,image,sha256);return result.ToArray();
  }
  public static object[] IconDiagnostics(Process process,uint pid,long created,string image,string sha256){
   var windows=Windows(process,pid,created,image,sha256,"tray_icon_app",false);var result=new List<object>();
   foreach(var window in windows){RequireWindow(process,pid,created,image,sha256,window,"tray_icon_app",false);
    for(uint id=1;id<=8;id++){var identifier=new IconIdentifier {Size=(uint)Marshal.SizeOf(typeof(IconIdentifier)),Window=window,Id=id,Guid=Guid.Empty};IconRect rect;int code=Shell_NotifyIconGetRect(ref identifier,out rect);result.Add(new {Window=window.ToInt64(),Id=id,HResult=code,Found=(code==0),Left=(code==0?rect.Left:0),Top=(code==0?rect.Top:0),Right=(code==0?rect.Right:0),Bottom=(code==0?rect.Bottom:0)});}
   }
   Require(process,pid,created,image,sha256);return result.ToArray();
  }
  public static IntPtr OpenMenu(Process process,uint pid,long created,string image,string sha256){
   var windows=Windows(process,pid,created,image,sha256,"tray_icon_app",false);if(windows.Length!=1)throw new InvalidOperationException("tray-window-not-unique");
   RequireWindow(process,pid,created,image,sha256,windows[0],"tray_icon_app",false);
   // Pinned tray-icon 0.24.2: actual shell callback message and right-button release.
   // This invokes its normal TrackPopupMenu path; no app.exit or WM_COMMAND is synthesized.
   LastPostError=0;if(!PostMessage(windows[0],6002,UIntPtr.Zero,new IntPtr(0x0205))){LastPostError=Marshal.GetLastWin32Error();throw new InvalidOperationException("tray-menu-open-failed");}return windows[0];
  }
 }
}
