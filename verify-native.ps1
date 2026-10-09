param([switch]$Rungui)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName Accessibility
$source=@"
using System; using System.Runtime.InteropServices; using System.Text;
public static class MinuterNativeTest {
public delegate bool EnumProc(IntPtr h,IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p,IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h,EnumProc p,IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
[DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder b,int c);
[DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder b,int c);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int n);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
[DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
[DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(IntPtr h,uint id,ref Guid iid,[MarshalAs(UnmanagedType.Interface)] out Accessibility.IAccessible accessible);
public static string Text(IntPtr h) {var b=new StringBuilder(2048);GetWindowText(h,b,2048);return b.ToString();}
public static IntPtr Window(uint pid,string name) {IntPtr found=IntPtr.Zero;EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);if(p==pid && Text(h)==name)found=h;return true;},IntPtr.Zero);return found;}
public static IntPtr Control(IntPtr window,string text) {IntPtr found=IntPtr.Zero;EnumChildWindows(window,(h,l)=>{if(Text(h)==text) found=h;return true;},IntPtr.Zero);return found;}
[DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessage(string name);
public static void Reopen(uint pid) {uint message=RegisterWindowMessage("wxTaskBarIconMessage");EnumWindows((h,l)=>{uint p;GetWindowThreadProcessId(h,out p);if(p==pid) {if(Text(h)=="") SendMessage(h,message,new IntPtr(99),new IntPtr(0x203));if(Text(h)=="Minuter — rungui experiment") SendMessage(h,0x8041,new IntPtr(1),new IntPtr(0x203));}return true;},IntPtr.Zero);}
[DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
public static IntPtr Combo(IntPtr window) {IntPtr found=IntPtr.Zero;EnumChildWindows(window,(h,l)=>{var b=new StringBuilder(64);GetClassName(h,b,64);if(b.ToString()=="ComboBox")found=h;return true;},IntPtr.Zero);return found;}
public static int ChoiceNotification=9;
public static void Select(IntPtr combo,int index) {SendMessage(combo,0x14E,new IntPtr(index),IntPtr.Zero);SendMessage(GetParent(combo),0x111,new IntPtr((ChoiceNotification<<16)|(GetDlgCtrlID(combo)&0xffff)),combo);}
public static string AccessibleName(IntPtr h) {return Access(h).get_accName(0);}
public static int AccessibleState(IntPtr h) {return Convert.ToInt32(Access(h).get_accState(0));}
public static string Class(IntPtr h) {var b=new StringBuilder(128);GetClassName(h,b,128);return b.ToString();}
public static int Role(IntPtr h) {return Convert.ToInt32(Access(h).get_accRole(0));}
public static Accessibility.IAccessible Access(IntPtr h) {var iid=new Guid("618736E0-3C3D-11CF-810C-00AA00389B71");Accessibility.IAccessible a;int hr=AccessibleObjectFromWindow(h,0xFFFFFFFC,ref iid,out a);if(hr!=0)throw new Exception("MSAA failed "+hr);return a;}
public static void Click(IntPtr h) {if(h==IntPtr.Zero)throw new Exception("Control missing");SendMessage(h,0xF5,IntPtr.Zero,IntPtr.Zero);}
public static bool Checked(IntPtr h) {return SendMessage(h,0xF0,IntPtr.Zero,IntPtr.Zero).ToInt64()==1;}
public static void Key(byte k) {keybd_event(k,0,0,UIntPtr.Zero);keybd_event(k,0,2,UIntPtr.Zero);}
public static void Shortcut() {keybd_event(0x11,0,0,UIntPtr.Zero);keybd_event(0x12,0,0,UIntPtr.Zero);keybd_event(0x10,0,0,UIntPtr.Zero);Key(8);keybd_event(0x10,0,2,UIntPtr.Zero);keybd_event(0x12,0,2,UIntPtr.Zero);keybd_event(0x11,0,2,UIntPtr.Zero);}
}
"@
Add-Type -TypeDefinition $source -ReferencedAssemblies Accessibility
function Assert($condition,$message) { if (!$condition) { throw $message }; "PASS: $message" }
$appFolder=if($Rungui){'Minuter-rungui'}else{'Minuter'}
$executable=if($Rungui){'target\release\minuter-rungui.exe'}else{'target\release\minuter.exe'}
$windowTitle=if($Rungui){'Minuter — rungui experiment'}else{'Minuter'}
if($Rungui){[MinuterNativeTest]::ChoiceNotification=1}
$file=Join-Path $env:APPDATA "$appFolder\settings.json"
$backup=if(Test-Path -LiteralPath $file){[System.IO.File]::ReadAllBytes($file)}else{$null}
$process=Start-Process (Join-Path $PSScriptRoot $executable) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $PSScriptRoot "target\native-test-stderr.log")
$nativeProcessHandle=$process.Handle
try {
Start-Sleep -Seconds 3
$window=[MinuterNativeTest]::Window($process.Id,$windowTitle)
Assert ($window -ne [IntPtr]::Zero) 'Settings window created'
Assert ([MinuterNativeTest]::Window($process.Id,'wxWidgets Debug Alert') -eq [IntPtr]::Zero) 'No native startup assertion'
[MinuterNativeTest]::ShowWindow($window,5)|Out-Null
[MinuterNativeTest]::SetForegroundWindow($window)|Out-Null
$tick=[MinuterNativeTest]::Control($window,'&Tick every second')
$close=[MinuterNativeTest]::Control($window,'&Close')
$quit=[MinuterNativeTest]::Control($window,'&Quit Minuter')
Assert ([MinuterNativeTest]::Role($tick) -eq 44) 'Tick is exposed as a checkbox'
Assert ([MinuterNativeTest]::AccessibleName($tick) -eq 'Tick every second') 'Tick has a descriptive accessible name'
"SETTINGS WINDOW CLASS: $([MinuterNativeTest]::Class($window))"
Assert ([MinuterNativeTest]::Role($close) -eq 43) 'Close is exposed as a button'
[MinuterNativeTest]::Click([MinuterNativeTest]::Control($window,'D&isable all'))
Assert (![MinuterNativeTest]::Checked($tick)) 'Disable all clears tick'
[MinuterNativeTest]::Click($tick)
Assert ([MinuterNativeTest]::Checked($tick)) 'Tick changes immediately'
Assert (([MinuterNativeTest]::AccessibleState($tick) -band 16) -ne 0) 'Checked state is exposed to screen readers'
Start-Sleep -Milliseconds 100
$saved=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json
Assert $saved.tick 'Cue settings are persisted'
$combo=[MinuterNativeTest]::Combo($window)
Assert ([MinuterNativeTest]::Role($combo) -eq 46) 'Sound device is exposed as a combo box'
Assert ([MinuterNativeTest]::AccessibleName($combo) -eq 'Sound device') 'Sound device has a descriptive accessible name'
"DEVICE COUNT: $([MinuterNativeTest]::SendMessage($combo,0x146,[IntPtr]::Zero,[IntPtr]::Zero))"
[MinuterNativeTest]::Select($combo,1)
Start-Sleep -Milliseconds 400
$saved=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json
"CURRENT DEVICE INDEX: $([MinuterNativeTest]::SendMessage($combo,0x147,[IntPtr]::Zero,[IntPtr]::Zero))"
"SAVED DEVICE: $($saved.device)"
Assert ($null -ne $saved.device) 'Selected output device is persisted'
[MinuterNativeTest]::Select($combo,0)
Start-Sleep -Milliseconds 400
$saved=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json
Assert ($null -eq $saved.device) 'System default output can be restored'
$speechStatus=[MinuterNativeTest]::Control($window,'Speech follows your screen reader or system voice.')
[MinuterNativeTest]::Click([MinuterNativeTest]::Control($window,'&Speak seconds'))
Start-Sleep -Milliseconds 1400
$message=[MinuterNativeTest]::Text($speechStatus)
Assert ($message.StartsWith('Speech: ')) "Prismer speech backend is active ($message)"
[MinuterNativeTest]::Click([MinuterNativeTest]::Control($window,'D&isable all'))
[MinuterNativeTest]::Click($close)
Assert (![MinuterNativeTest]::IsWindowVisible($window)) 'Close hides settings'
Assert (!$process.HasExited) 'Closing keeps the app running'
[MinuterNativeTest]::Shortcut()
Start-Sleep -Milliseconds 300
Assert ([MinuterNativeTest]::IsWindowVisible($window)) 'Ctrl+Alt+Shift+Backspace shows settings'
[MinuterNativeTest]::Shortcut()
Start-Sleep -Milliseconds 300
Assert (![MinuterNativeTest]::IsWindowVisible($window)) 'Ctrl+Alt+Shift+Backspace hides settings'
Assert ([MinuterNativeTest]::Control($window,'Speech uses Prismer.') -eq [IntPtr]::Zero) 'Dialog does not mention Prismer'
[MinuterNativeTest]::Reopen($process.Id)
Start-Sleep -Milliseconds 200
Assert ([MinuterNativeTest]::IsWindowVisible($window)) 'Tray activation reopens settings'
[MinuterNativeTest]::SetForegroundWindow($window)|Out-Null
[MinuterNativeTest]::Key(27)
Start-Sleep -Milliseconds 200
Assert (![MinuterNativeTest]::IsWindowVisible($window)) 'Escape hides settings'
[MinuterNativeTest]::Reopen($process.Id)
Start-Sleep -Milliseconds 200
[MinuterNativeTest]::SetForegroundWindow($window)|Out-Null
Start-Sleep -Milliseconds 300
[MinuterNativeTest]::Key(13)
Start-Sleep -Milliseconds 200
Assert (![MinuterNativeTest]::IsWindowVisible($window)) 'Enter activates Close'
[MinuterNativeTest]::Reopen($process.Id)
Start-Sleep -Milliseconds 200
[MinuterNativeTest]::Click($quit)
Assert ($process.WaitForExit(10000)) 'Quit exits cleanly'
"EXIT STATUS: $($process.ExitCode)"
Assert ($process.ExitCode -eq 0) 'Exit status is successful'
} finally {
if (!$process.HasExited) {Stop-Process -Id $process.Id}
if ($null -ne $backup) {[System.IO.File]::WriteAllBytes($file,$backup)} elseif (Test-Path -LiteralPath $file) {Remove-Item -LiteralPath $file}
}
