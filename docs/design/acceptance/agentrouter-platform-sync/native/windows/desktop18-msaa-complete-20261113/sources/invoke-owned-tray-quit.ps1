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
Add-Type -AssemblyName Accessibility
Add-Type -Path (Join-Path $PSScriptRoot 'NativeTrayQuit.cs') -ReferencedAssemblies @('System.dll', [Accessibility.IAccessible].Assembly.Location)
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
 [Orchester.FinalDesktop.NativeTray]::RequireOwnedDefaultPopup($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
 Write-JsonFile ([ordered]@{event='owned_popup_thread_diagnostics';records=@([Orchester.FinalDesktop.NativeTray]::ThreadDiagnostics($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,'#32768'));readOnly=$true;modelCalls=0}) (Join-Path $ResultDirectory 'popup-thread-diagnostic.json')

 # UIA uses its own deadline; time spent waiting for the popup is not consumed here.
 $script:trayStage='native-popup-accessibility';$pattern=$null;$itemCount=$null;$lastUiaErrorType=$null;$uiaLimit=[DateTime]::UtcNow.AddSeconds(5)
 do{
  [Orchester.FinalDesktop.NativeTray]::RequireOwnedDefaultPopup($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
  $labels=@();$quit=$null;$pattern=$null
  try{
   $element=[System.Windows.Automation.AutomationElement]::FromHandle($popup)
   if($element.Current.ProcessId -ne $ExpectedProcessId){throw [InvalidOperationException]::new('tray-popup-uia-owner-mismatch')}
   $condition=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::MenuItem)
   $items=$element.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition);$itemCount=$items.Count
   foreach($item in $items){$labels+=$item.Current.Name;if($item.Current.Name -ceq 'Quit Orchester'){$quit=$item}}
   if($labels.Count -eq 3 -and $labels[0] -ceq 'Open Orchester' -and $labels[1] -ceq 'Settings...' -and $labels[2] -ceq 'Quit Orchester' -and $quit -and $quit.Current.IsEnabled -and -not $quit.Current.IsOffscreen -and $quit.Current.ProcessId -eq $ExpectedProcessId){$null=$quit.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)}
  }catch{
   $base=$_.Exception.GetBaseException();if($base.Message -ceq 'tray-popup-uia-owner-mismatch'){throw};$lastUiaErrorType=$base.GetType().FullName
  }
  if(-not $pattern){Start-Sleep -Milliseconds 50}
 }while(-not $pattern -and [DateTime]::UtcNow -lt $uiaLimit)
 if($pattern){
  # Read the same owned popup again. Use only the second actual Quit element.
  $firstLabels=@($labels);$script:trayStage='native-uia-second-read'
  [Orchester.FinalDesktop.NativeTray]::RequireOwnedDefaultPopup($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
  $element=[System.Windows.Automation.AutomationElement]::FromHandle($popup)
  if($element.Current.ProcessId -ne $ExpectedProcessId){throw [InvalidOperationException]::new('tray-popup-uia-owner-mismatch')}
  $items=$element.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition);$labels=@();$quit=$null;$pattern=$null
  foreach($item in $items){$labels+=$item.Current.Name;if(-not $item.Current.IsEnabled -or $item.Current.IsOffscreen -or $item.Current.ProcessId -ne $ExpectedProcessId){throw [InvalidOperationException]::new('tray-native-menu-items-differ')};if($item.Current.Name -ceq 'Quit Orchester'){$quit=$item}}
  if($labels.Count -ne 3 -or $labels[0] -cne 'Open Orchester' -or $labels[1] -cne 'Settings...' -or $labels[2] -cne 'Quit Orchester' -or ($labels -join ',') -cne ($firstLabels -join ',') -or -not $quit){throw [InvalidOperationException]::new('tray-native-menu-items-differ')}
  if(-not $quit.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){throw [InvalidOperationException]::new('tray-native-quit-not-invokable')}
  [Orchester.FinalDesktop.NativeTray]::RequireOwnedDefaultPopup($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
  Write-JsonFile ([ordered]@{event='owned_native_tray_menu_observed';guiPid=$ExpectedProcessId;guiCreated=$ExpectedCreated;binarySha256=$ExpectedSha256;trayHwnd=$tray.ToInt64();popupHwnd=$popup.ToInt64();nativeClass='#32768';labels=$labels;selected='Quit Orchester';activation='UIAutomation.InvokePattern';twoIndependentStrictReadsVerified=$true;defaultDesktopVerified=$true;originalProcessHandleRetained=$true;noForcedProcessTermination=$true;modelCalls=0}) (Join-Path $ResultDirectory 'tray-menu-observation.json')
  $script:trayStage='native-quit-invoke';([System.Windows.Automation.InvokePattern]$pattern).Invoke()
 }else{
  # Preserve the original UIA failure separately; no unknown label text is written.
  Write-JsonFile ([ordered]@{status='uia-menu-unavailable-before-msaa';stage='native-popup-accessibility';popupCount=$popupCount;itemCount=$itemCount;lastUiaErrorType=$lastUiaErrorType;knownMenuLabels=@($labels|Where-Object{$_ -cin @('Open Orchester','Settings...','Quit Orchester')});unknownLabelCount=@($labels|Where-Object{$_ -cnotin @('Open Orchester','Settings...','Quit Orchester')}).Count;menuInvoked=$false;modelCalls=0}) (Join-Path $ResultDirectory 'tray-uia-before-msaa-safe.json')
  $script:trayStage='native-popup-msaa-read';$msaaRead=$null;$msaaLimit=[DateTime]::UtcNow.AddSeconds(5)
  do{
   [Orchester.FinalDesktop.NativeTray]::RequireOwnedDefaultPopup($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
   $msaaRead=[Orchester.FinalDesktop.NativeTray]::ReadStrictMsaaMenu($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
   if(-not $msaaRead.StrictLabelsVerified){Start-Sleep -Milliseconds 50}
  }while(-not $msaaRead.StrictLabelsVerified -and [DateTime]::UtcNow -lt $msaaLimit)
  Write-JsonFile ([ordered]@{event='owned_native_msaa_menu_read';guiPid=$ExpectedProcessId;guiCreatedFileTimeExact=$ExpectedCreated.ToString();binarySha256=$ExpectedSha256;popupHwnd=$popup.ToInt64();read=$msaaRead;menuInvoked=$false;modelCalls=0}) (Join-Path $ResultDirectory 'tray-msaa-first-read-safe.json')
  if(-not $msaaRead.StrictLabelsVerified){throw [InvalidOperationException]::new('tray-msaa-strict-menu-unverified')}
  # The C# method performs two fresh strict reads and rechecks exact identity,
  # popup ownership and Default/input desktop before accDoDefaultAction.
  $script:trayStage='native-msaa-quit-invoke'
  $invoked=[Orchester.FinalDesktop.NativeTray]::InvokeStrictMsaaQuit($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256,$popup)
  Write-JsonFile ([ordered]@{event='owned_native_tray_menu_observed';guiPid=$ExpectedProcessId;guiCreatedFileTimeExact=$ExpectedCreated.ToString();binarySha256=$ExpectedSha256;trayHwnd=$tray.ToInt64();popupHwnd=$popup.ToInt64();nativeClass='#32768';labels=$invoked.Labels;selected='Quit Orchester';activation='MSAA.IAccessible.accDoDefaultAction';read=$invoked;originalProcessHandleRetained=$true;noForcedProcessTermination=$true;actualGuiNativeExitCode=$null;modelCalls=0}) (Join-Path $ResultDirectory 'tray-menu-observation.json')
  if(-not $invoked.ActionReturned -and -not $invoked.GuiHeldHandleSignalledAfterAction){throw [InvalidOperationException]::new('tray-msaa-action-return-unavailable')}
 }

}catch{
 $base=$_.Exception.GetBaseException();$allowed=@('tray-process-unavailable','tray-process-identity-mismatch','tray-window-owner-mismatch','tray-window-enumeration-failed','tray-window-not-unique','tray-menu-open-failed','tray-popup-not-unique','tray-popup-uia-owner-mismatch','tray-native-menu-items-differ','tray-native-quit-not-invokable','tray-msaa-default-desktop-unproved','tray-msaa-strict-menu-unverified','tray-msaa-two-reads-unverified','tray-msaa-read-binding-changed','tray-msaa-retained-identity-unavailable','tray-msaa-action-return-unavailable')
 Write-JsonFile ([ordered]@{status='failed';stage=$script:trayStage;type=$base.GetType().FullName;staticCode=if($base.Message -cin $allowed){$base.Message}else{$null};popupCount=$popupCount;postMessageError=[Orchester.FinalDesktop.NativeTray]::LastPostError;itemCount=if(Get-Variable itemCount -ErrorAction SilentlyContinue){$itemCount}else{$null};unknownLabelCount=@($labels|Where-Object{$_ -cnotin @('Open Orchester','Settings...','Quit Orchester')}).Count;knownMenuLabels=@($labels|Where-Object{$_ -cin @('Open Orchester','Settings...','Quit Orchester')});menuInvocationAttempted=($script:trayStage -cin @('native-quit-invoke','native-msaa-quit-invoke'));menuInvoked=if(Get-Variable invoked -ErrorAction SilentlyContinue){$invoked.ActionInvoked}else{$false};modelCalls=0}) (Join-Path $ResultDirectory 'tray-helper-failure.json')
 throw
}
