$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$root='C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414';$source=Join-Path $root '05-successor-default-runtime.ps1'
if((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'dc2a2bfeb0b6e19d86d6cf34d2aebd47281eb71040f40e03b0841879eea11f02'){throw 'successor-source-pin-differs'}
$acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$rule=[Security.AccessControl.FileSystemAccessRule]::new($sid,[Security.AccessControl.FileSystemRights]::FullControl,[Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit',[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Allow);$acl.AddAccessRule($rule)}
Set-Acl -LiteralPath $root -AclObject $acl
if(-not (Get-Acl -LiteralPath $root).AreAccessRulesProtected){throw 'private-output-inheritance-not-disabled'}
& $source -ExpectedFreshBackupSafeSha256 'c2277a82ccf1df70c61ac18190fdc803801053f54d36dcbdcff06febd89a090d' -RuntimeTimeoutSeconds 600
exit $LASTEXITCODE
