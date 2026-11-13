param(
 [Parameter(Mandatory=$true)][Diagnostics.Process]$Process,
 [Parameter(Mandatory=$true)][uint32]$ExpectedProcessId,
 [Parameter(Mandatory=$true)][long]$ExpectedCreated,
 [Parameter(Mandatory=$true)][string]$ExpectedImage,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedSha256,
 [Parameter(Mandatory=$true)][string]$ResultDirectory
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
Add-Type -Path (Join-Path $PSScriptRoot 'NativeTrayQuit.cs')
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$script:trayStage='owned-tray-window';$tray=$null;$popup=$null;$labels=@();$popupCount=0
try{
 $trayWindows=[Orchester.FinalDesktop.NativeTray]::Windows($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,'tray_icon_app',$false)
 Write-JsonFile ([ordered]@{event='owned_tray_host_observed';guiPid=$ExpectedProcessId;guiCreated=$ExpectedCreated;binarySha256=$ExpectedSha256;trayWindowCount=$trayWindows.Length;guiSessionId=$Process.SessionId;harnessSessionId=[Diagnostics.Process]::GetCurrentProcess().SessionId;modelCalls=0}) (Join-Path $ResultDirectory 'tray-host-observation.json')
 Write-JsonFile ([ordered]@{event='owned_tray_icon_rect_diagnostics';records=@([Orchester.FinalDesktop.NativeTray]::IconDiagnostics($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256));readOnly=$true;modelCalls=0}) (Join-Path $ResultDirectory 'tray-icon-rect-diagnostic.json')
 Write-JsonFile ([ordered]@{event='owned_tray_thread_diagnostics';records=@([Orchester.FinalDesktop.NativeTray]::ThreadDiagnostics($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,'tray_icon_app'));readOnly=$true;modelCalls=0}) (Join-Path $ResultDirectory 'tray-thread-diagnostic.json')
 $script:trayStage='native-tray-callback';$tray=[Orchester.FinalDesktop.NativeTray]::OpenMenu($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256)
 $script:trayStage='owned-popup-window';$limit=[DateTime]::UtcNow.AddSeconds(5);$popups=@()
 do{$popups=@([Orchester.FinalDesktop.NativeTray]::Windows($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,'#32768',$true));if($popups.Count -eq 0){Start-Sleep -Milliseconds 50}}while($popups.Count -eq 0 -and [DateTime]::UtcNow -lt $limit)
 $popupCount=$popups.Count
 if($popupCount -ne 1){throw [InvalidOperationException]::new('tray-popup-not-unique')}
 $popup=$popups[0]
 [Orchester.FinalDesktop.NativeTray]::RequireWindow($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup,'#32768',$true)
 Write-JsonFile ([ordered]@{event='owned_popup_thread_diagnostics';records=@([Orchester.FinalDesktop.NativeTray]::ThreadDiagnostics($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,'#32768'));readOnly=$true;modelCalls=0}) (Join-Path $ResultDirectory 'popup-thread-diagnostic.json')
 $script:trayStage='native-popup-accessibility';$pattern=$null;$itemCount=0;$lastUiaErrorType=$null
 do{
  [Orchester.FinalDesktop.NativeTray]::RequireWindow($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup,'#32768',$true)
  $labels=@();$quit=$null
  try{
   $element=[System.Windows.Automation.AutomationElement]::FromHandle($popup)
   if($element.Current.ProcessId -ne $ExpectedProcessId){throw [InvalidOperationException]::new('tray-popup-uia-owner-mismatch')}
   $condition=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::MenuItem)
   $items=$element.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition);$itemCount=$items.Count
   foreach($item in $items){$labels+=$item.Current.Name;if($item.Current.Name -ceq 'Quit Orchester'){$quit=$item}}
   if(($labels -join ',') -ceq 'Open Orchester,Settings...,Quit Orchester' -and $quit -and $quit.Current.IsEnabled -and $quit.Current.ProcessId -eq $ExpectedProcessId){$null=$quit.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)}
  }catch{
   $base=$_.Exception.GetBaseException();if($base.Message -ceq 'tray-popup-uia-owner-mismatch'){throw};$lastUiaErrorType=$base.GetType().FullName
  }
  if(-not $pattern){Start-Sleep -Milliseconds 50}
 }while(-not $pattern -and [DateTime]::UtcNow -lt $limit)
 if(($labels -join ',') -cne 'Open Orchester,Settings...,Quit Orchester' -or -not $quit -or -not $quit.Current.IsEnabled -or $quit.Current.ProcessId -ne $ExpectedProcessId){throw [InvalidOperationException]::new('tray-native-menu-items-differ')}
 if(-not $pattern){throw [InvalidOperationException]::new('tray-native-quit-not-invokable')}
 [Orchester.FinalDesktop.NativeTray]::RequireWindow($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup,'#32768',$true)
 Write-JsonFile ([ordered]@{event='owned_native_tray_menu_observed';guiPid=$ExpectedProcessId;guiCreated=$ExpectedCreated;binarySha256=$ExpectedSha256;trayHwnd=$tray.ToInt64();popupHwnd=$popup.ToInt64();nativeClass='#32768';labels=$labels;selected='Quit Orchester';activation='UIAutomation.InvokePattern';originalProcessHandleRetained=$true;noForcedProcessTermination=$true;modelCalls=0}) (Join-Path $ResultDirectory 'tray-menu-observation.json')
 $script:trayStage='native-quit-invoke';([System.Windows.Automation.InvokePattern]$pattern).Invoke()
}catch{
 $base=$_.Exception.GetBaseException();$allowed=@('tray-process-unavailable','tray-process-identity-mismatch','tray-window-owner-mismatch','tray-window-enumeration-failed','tray-window-not-unique','tray-menu-open-failed','tray-popup-not-unique','tray-popup-uia-owner-mismatch','tray-native-menu-items-differ','tray-native-quit-not-invokable')
 Write-JsonFile ([ordered]@{status='failed';stage=$script:trayStage;type=$base.GetType().FullName;staticCode=if($base.Message -cin $allowed){$base.Message}else{$null};popupCount=$popupCount;postMessageError=[Orchester.FinalDesktop.NativeTray]::LastPostError;itemCount=if(Get-Variable itemCount -ErrorAction SilentlyContinue){$itemCount}else{0};unknownLabelCount=@($labels|Where-Object{$_ -cnotin @('Open Orchester','Settings...','Quit Orchester')}).Count;knownMenuLabels=@($labels|Where-Object{$_ -cin @('Open Orchester','Settings...','Quit Orchester')});menuInvoked=($script:trayStage -ceq 'native-quit-invoke');modelCalls=0}) (Join-Path $ResultDirectory 'tray-helper-failure.json')
 throw
}
