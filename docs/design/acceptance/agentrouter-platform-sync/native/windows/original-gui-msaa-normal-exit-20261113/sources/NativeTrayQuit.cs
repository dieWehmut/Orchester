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

  public sealed class MsaaMenuRead {
   public int? HResult {get;set;}
   public int? RootRole {get;set;}
   public int? ItemCount {get;set;}
   public int? ChildrenObtained {get;set;}
   public int? QuitRole {get;set;}
   public int? QuitState {get;set;}
   public string[] Labels {get;set;}
   public int? KnownLabelCount {get;set;}
   public int? UnknownLabelCount {get;set;}
   public bool ReadComplete {get;set;}
   public bool StrictLabelsVerified {get;set;}
   public bool DefaultDesktopVerified {get;set;}
   public bool QuitDefaultActionAvailable {get;set;}
   public bool TwoIndependentStrictReadsVerified {get;set;}
   public bool ActionInvoked {get;set;}
   public bool ActionAttempted {get;set;}
   public bool ActionReturned {get;set;}
   public int? ActionHResult {get;set;}
   public string ActionErrorType {get;set;}
   public bool GuiHeldHandleSignalledAfterAction {get;set;}
   public string SafeReadErrorType {get;set;}
   public string StaticCode {get;set;}
  }
  sealed class MsaaContext : IDisposable {
   public MsaaMenuRead Read=new MsaaMenuRead();
   public Accessibility.IAccessible Root;
   public Accessibility.IAccessible Quit;
   public object QuitChild;
   public string[] ChildKinds;
   public int[] SimpleChildIds;
   readonly List<object> references=new List<object>();
   public void Hold(object value){if(value!=null&&Marshal.IsComObject(value))references.Add(value);}
   public void Dispose(){foreach(var value in references){try{Marshal.ReleaseComObject(value);}catch(InvalidComObjectException){}}references.Clear();}
  }
  [DllImport("oleacc.dll")] static extern int AccessibleObjectFromWindow(IntPtr hwnd,uint objectId,ref Guid iid,[MarshalAs(UnmanagedType.Interface)]out Accessibility.IAccessible accessible);
  [DllImport("oleacc.dll")] static extern int WindowFromAccessibleObject([MarshalAs(UnmanagedType.Interface)]Accessibility.IAccessible accessible,out IntPtr hwnd);
  [DllImport("oleacc.dll")] static extern int AccessibleChildren([MarshalAs(UnmanagedType.Interface)]Accessibility.IAccessible accessible,int start,int count,[Out,MarshalAs(UnmanagedType.LPArray,ArraySubType=UnmanagedType.Struct,SizeParamIndex=2)]object[] children,out int obtained);
  [DllImport("user32.dll",SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
  [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
  static string HandleDesktopName(IntPtr desktop){var name=new StringBuilder(256);uint needed;return desktop!=IntPtr.Zero&&GetUserObjectInformation(desktop,2,name,512,out needed)?name.ToString():null;}
  static void RequireDefaultDesktop(Process process,uint pid,long created,string image,string sha256,IntPtr popup){
   RequireWindow(process,pid,created,image,sha256,popup,"#32768",true);
   uint owner;uint popupThread=GetWindowThreadProcessId(popup,out owner);
   using(var self=Process.GetCurrentProcess()){if(process.SessionId!=self.SessionId)throw new InvalidOperationException("tray-msaa-session-differs");}
   var input=OpenInputDesktop(0,false,1);
   try {if(owner!=pid||popupThread==0||DesktopName(popupThread)!="Default"||DesktopName(GetCurrentThreadId())!="Default"||HandleDesktopName(input)!="Default")throw new InvalidOperationException("tray-msaa-default-desktop-unproved");}
   finally {if(input!=IntPtr.Zero)CloseDesktop(input);}
   RequireWindow(process,pid,created,image,sha256,popup,"#32768",true);
  }
  public static void RequireOwnedDefaultPopup(Process process,uint pid,long created,string image,string sha256,IntPtr popup){RequireDefaultDesktop(process,pid,created,image,sha256,popup);}
  static void RequireAccessibleWindow(Accessibility.IAccessible accessible,IntPtr popup){IntPtr actual;int hr=WindowFromAccessibleObject(accessible,out actual);if(hr!=0||actual!=popup)throw new InvalidOperationException("tray-msaa-accessible-window-differs");}
  static int ActualInt(object value){if(!(value is int))throw new InvalidOperationException("tray-msaa-integer-property-unavailable");return (int)value;}
  static bool IsExpectedLabel(string label){return label=="Open Orchester"||label=="Settings..."||label=="Quit Orchester";}
  static MsaaContext ReadMsaaCore(Process process,uint pid,long created,string image,string sha256,IntPtr popup){
   var context=new MsaaContext();var read=context.Read;read.Labels=new string[0];
   try {
    RequireDefaultDesktop(process,pid,created,image,sha256,popup);read.DefaultDesktopVerified=true;
    var iid=new Guid("618736e0-3c3d-11cf-810c-00aa00389b71");Accessibility.IAccessible root;
    int hr=AccessibleObjectFromWindow(popup,unchecked((uint)-4),ref iid,out root);read.HResult=hr;
    context.Root=root;context.Hold(root);
    if(hr!=0||root==null)throw new InvalidOperationException("tray-msaa-root-unavailable");
    RequireAccessibleWindow(root,popup);
    read.RootRole=ActualInt(root.get_accRole(0));
    if(read.RootRole!=11)throw new InvalidOperationException("tray-msaa-root-role-differs");
    read.ItemCount=root.accChildCount;
    if(read.ItemCount!=3)throw new InvalidOperationException("tray-msaa-item-count-differs");
    var children=new object[3];int obtained;
    hr=AccessibleChildren(root,0,3,children,out obtained);read.HResult=hr;read.ChildrenObtained=obtained;
    foreach(var child in children)context.Hold(child);
    if(hr!=0||obtained!=3)throw new InvalidOperationException("tray-msaa-children-unavailable");
    var labels=new List<string>();var childKinds=new List<string>();var childIds=new List<int>();var known=new List<string>();var seenIds=new List<int>();int unknown=0;
    string[] expected={"Open Orchester","Settings...","Quit Orchester"};bool strict=true;
    for(int n=0;n<3;n++){
     Accessibility.IAccessible item;object childId;
     if(children[n] is int){int actualId=(int)children[n];if(actualId<=0||seenIds.Contains(actualId))throw new InvalidOperationException("tray-msaa-child-id-differs");seenIds.Add(actualId);item=root;childId=actualId;childKinds.Add("simple-child");childIds.Add(actualId);}
     else {item=children[n] as Accessibility.IAccessible;if(item==null)throw new InvalidOperationException("tray-msaa-child-interface-unavailable");childId=0;RequireAccessibleWindow(item,popup);childKinds.Add("accessible-child");childIds.Add(0);}
     var label=item.get_accName(childId);int role=ActualInt(item.get_accRole(childId));int state=ActualInt(item.get_accState(childId));
     labels.Add(label);
     if(IsExpectedLabel(label))known.Add(label);else unknown++;
     if(label!=expected[n]||role!=12||(state&(1|0x8000|0x10000|0x400000))!=0)strict=false;
     if(n==2){context.Quit=item;context.QuitChild=childId;read.QuitRole=role;read.QuitState=state;read.QuitDefaultActionAvailable=!String.IsNullOrWhiteSpace(item.get_accDefaultAction(childId));if(!read.QuitDefaultActionAvailable)strict=false;}
    }
    read.Labels=known.ToArray();read.KnownLabelCount=known.Count;read.UnknownLabelCount=unknown;read.ReadComplete=true;
    context.ChildKinds=childKinds.ToArray();context.SimpleChildIds=childIds.ToArray();
    RequireAccessibleWindow(root,popup);RequireDefaultDesktop(process,pid,created,image,sha256,popup);
    read.StrictLabelsVerified=strict&&unknown==0&&labels.Count==3;
    if(!read.StrictLabelsVerified)read.StaticCode="tray-msaa-strict-menu-differs";
   }catch(Exception error){read.SafeReadErrorType=error.GetType().FullName;var com=error as COMException;if(com!=null)read.HResult=com.ErrorCode;read.StrictLabelsVerified=false;read.StaticCode=error is InvalidOperationException&&error.Message.StartsWith("tray-msaa-",StringComparison.Ordinal)?error.Message:"tray-msaa-read-unavailable";}
   return context;
  }
  public static MsaaMenuRead ReadStrictMsaaMenu(Process process,uint pid,long created,string image,string sha256,IntPtr popup){using(var context=ReadMsaaCore(process,pid,created,image,sha256,popup)){return context.Read;}}
  static void RequireSameRead(MsaaContext first,MsaaContext second){
   if(!first.Read.StrictLabelsVerified||!second.Read.StrictLabelsVerified||first.ChildKinds.Length!=3||second.ChildKinds.Length!=3)throw new InvalidOperationException("tray-msaa-two-reads-unverified");
   for(int n=0;n<3;n++)if(first.Read.Labels[n]!=second.Read.Labels[n]||first.ChildKinds[n]!=second.ChildKinds[n]||first.SimpleChildIds[n]!=second.SimpleChildIds[n])throw new InvalidOperationException("tray-msaa-read-binding-changed");
  }
  public static MsaaMenuRead InvokeStrictMsaaQuit(Process process,uint pid,long created,string image,string sha256,IntPtr popup){
   using(var first=ReadMsaaCore(process,pid,created,image,sha256,popup))using(var second=ReadMsaaCore(process,pid,created,image,sha256,popup)){
    RequireSameRead(first,second);RequireAccessibleWindow(second.Quit,popup);RequireDefaultDesktop(process,pid,created,image,sha256,popup);
    second.Read.TwoIndependentStrictReadsVerified=true;
    second.Read.ActionAttempted=true;
    try {second.Quit.accDoDefaultAction(second.QuitChild);second.Read.ActionReturned=true;second.Read.ActionInvoked=true;second.Read.ActionHResult=0;}
    catch(COMException error){second.Read.ActionReturned=false;second.Read.ActionInvoked=false;second.Read.ActionHResult=error.ErrorCode;second.Read.ActionErrorType=error.GetType().FullName;}
    finally {
     // Always recheck the same held identity, even if the action disconnected COM.
     // Normal Quit may already have completed, so a signalled held handle is allowed.
     var retained=process.Handle;FileTime c,e,k,u;uint wait=WaitForSingleObject(retained,0);
     if(GetProcessId(retained)!=pid||!GetProcessTimes(retained,out c,out e,out k,out u)||(((long)c.High<<32)|c.Low)!=created||(wait!=0&&wait!=258))throw new InvalidOperationException("tray-msaa-retained-identity-unavailable");
     second.Read.GuiHeldHandleSignalledAfterAction=(wait==0);
     if(wait==258)Require(process,pid,created,image,sha256);
    }
    return second.Read;
   }
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
