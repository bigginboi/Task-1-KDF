$ErrorActionPreference = "Stop"

$apiUrl = "http://localhost:3001"

Write-Host "=== Remote Build Utility PowerShell Integration Tests ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Health check
Write-Host "[1/5] Testing server health..." -NoNewline
try {
    $health = Invoke-RestMethod -Uri "$apiUrl/health" -Method Get
    if ($health -eq "OK") {
        Write-Host " [PASSED]" -ForegroundColor Green
    } else {
        Write-Host " [FAILED] (Returned: $health)" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host " [FAILED] ($($_.Exception.Message))" -ForegroundColor Red
    exit 1
}

# Test 2: Valid C++ sync & compile
Write-Host "[2/5] Testing valid C++ sync & compile..." -NoNewline
try {
    $cppFilePath = Join-Path $PSScriptRoot "test-project\main.cpp"
    if (-not (Test-Path $cppFilePath)) {
        $base64Content = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes("#include <iostream>`nint main() { std::cout << ""Hello"" << std::endl; return 0; }"))
    } else {
        $base64Content = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($cppFilePath))
    }

    $body = @{
        files = @(
            @{
                path = "main.cpp"
                content = $base64Content
            }
        )
    } | ConvertTo-Json -Depth 3

    # Sync
    $syncResp = Invoke-RestMethod -Uri "$apiUrl/api/sync?source_type=cpp" -Method Post -Body $body -ContentType "application/json"
    $workspaceId = $syncResp.workspace_id

    # Compile
    $compileResp = Invoke-RestMethod -Uri "$apiUrl/api/compile?workspace_id=$workspaceId&source_type=cpp" -Method Put

    if ($compileResp.success -and $compileResp.status_code -eq 0) {
        Write-Host " [PASSED] (Workspace: $workspaceId)" -ForegroundColor Green
    } else {
        Write-Host " [FAILED] (Success: $($compileResp.success), Code: $($compileResp.status_code))" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host " [FAILED] ($($_.Exception.Message))" -ForegroundColor Red
    exit 1
}

# Test 3: Path traversal protection
Write-Host "[3/5] Testing path traversal prevention..." -NoNewline
try {
    $body = @{
        files = @(
            @{
                path = "../../etc/passwd"
                content = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes("test"))
            }
        )
    } | ConvertTo-Json -Depth 3

    $syncResp = Invoke-RestMethod -Uri "$apiUrl/api/sync?source_type=cpp" -Method Post -Body $body -ContentType "application/json"
    Write-Host " [FAILED] (Allowed invalid path traversal)" -ForegroundColor Red
    exit 1
} catch {
    $statusCode = [int]$_.Exception.Response.StatusCode
    $responseBody = $_.ErrorDetails.Message | ConvertFrom-Json

    if ($statusCode -eq 403 -and $responseBody.error_code -eq "INVALID_PATH") {
        Write-Host " [PASSED] (Blocked with 403 / INVALID_PATH)" -ForegroundColor Green
    } else {
        Write-Host " [FAILED] (Status: $statusCode, Error Code: $($responseBody.error_code))" -ForegroundColor Red
        exit 1
    }
}

# Test 4: Payload limit enforcement
Write-Host "[4/5] Testing payload size limits..." -NoNewline
try {
    # Generate a dummy large content (>50MB) in memory
    $largeBytes = New-Object Byte[] (51 * 1024 * 1024)
    $largeBase64 = [Convert]::ToBase64String($largeBytes)

    $body = @{
        files = @(
            @{
                path = "large.bin"
                content = $largeBase64
            }
        )
    } | ConvertTo-Json -Depth 3

    $syncResp = Invoke-RestMethod -Uri "$apiUrl/api/sync?source_type=cpp" -Method Post -Body $body -ContentType "application/json"
    Write-Host " [FAILED] (Allowed payload > 50MB)" -ForegroundColor Red
    exit 1
} catch {
    $statusCode = [int]$_.Exception.Response.StatusCode
    $responseBody = $_.ErrorDetails.Message | ConvertFrom-Json

    if ($statusCode -eq 507 -and $responseBody.error_code -eq "PAYLOAD_TOO_LARGE") {
        Write-Host " [PASSED] (Blocked with 507 / PAYLOAD_TOO_LARGE)" -ForegroundColor Green
    } else {
        Write-Host " [FAILED] (Status: $statusCode, Error Code: $($responseBody.error_code))" -ForegroundColor Red
        exit 1
    }
}

# Test 5: Rate limiting
Write-Host "[5/5] Testing rate limiter..." -NoNewline
try {
    $body = @{ files = @() } | ConvertTo-Json
    $rateLimited = $false
    for ($i = 1; $i -le 15; $i++) {
        try {
            $syncResp = Invoke-RestMethod -Uri "$apiUrl/api/sync?source_type=cpp" -Method Post -Body $body -ContentType "application/json"
        } catch {
            $statusCode = [int]$_.Exception.Response.StatusCode
            $responseBody = $_.ErrorDetails.Message | ConvertFrom-Json

            if ($statusCode -eq 429 -and $responseBody.error_code -eq "RATE_LIMIT_EXCEEDED") {
                $rateLimited = $true
                break
            } else {
                throw $_
            }
        }
    }

    if ($rateLimited) {
        Write-Host " [PASSED] (Rate limited on request $i)" -ForegroundColor Green
    } else {
        Write-Host " [FAILED] (Limiter did not activate after 15 requests)" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host " [FAILED] ($($_.Exception.Message))" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== Integration Tests Completed Successfully ===" -ForegroundColor Green
