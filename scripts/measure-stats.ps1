# Shared stats for DialKey n=10 gates (dot-source from measure-*.ps1).
# Unbiased sample SD: σ̂ = sqrt(SS/(n-1)). α = 0.05.
# Comparisons: Shapiro–Wilk (normality), F-test (equal variance), Student / Welch t.

function Get-UnbiasedStats([double[]]$xs) {
    $n = @($xs).Count
    if ($n -lt 1) {
        throw "Get-UnbiasedStats: empty sample"
    }
    $mean = ($xs | Measure-Object -Average).Average
    $ss = 0.0
    foreach ($x in $xs) { $ss += ($x - $mean) * ($x - $mean) }
    $var = if ($n -gt 1) { $ss / ($n - 1) } else { 0.0 }
    [pscustomobject]@{
        N    = $n
        Mean = $mean
        SS   = $ss
        Var  = $var
        SD   = [math]::Sqrt($var)
        Min  = ($xs | Measure-Object -Minimum).Minimum
        Max  = ($xs | Measure-Object -Maximum).Maximum
    }
}

# Shapiro–Wilk coefficients a_1..a_{floor(n/2)} (original tables, n=5..12).
function Get-ShapiroWilkCoeffs([int]$n) {
    switch ($n) {
        5 { return @(0.6646, 0.2413) }
        6 { return @(0.6431, 0.2806, 0.0875) }
        7 { return @(0.6233, 0.3031, 0.1401) }
        8 { return @(0.6052, 0.3164, 0.1743, 0.0561) }
        9 { return @(0.5888, 0.3244, 0.1976, 0.0947) }
        10 { return @(0.5739, 0.3291, 0.2141, 0.1224, 0.0399) }
        11 { return @(0.5601, 0.3315, 0.2260, 0.1429, 0.0695) }
        12 { return @(0.5475, 0.3325, 0.2347, 0.1586, 0.0922, 0.0303) }
        default { return $null }
    }
}

# W(0.05) critical values: reject normality if W < Wcrit.
function Get-ShapiroWilkWcrit05([int]$n) {
    switch ($n) {
        5 { return 0.762 }
        6 { return 0.788 }
        7 { return 0.803 }
        8 { return 0.818 }
        9 { return 0.829 }
        10 { return 0.842 }
        11 { return 0.850 }
        12 { return 0.859 }
        default { return $null }
    }
}

function Get-ShapiroWilk([double[]]$xs) {
    $st = Get-UnbiasedStats $xs
    $n = $st.N
    $a = Get-ShapiroWilkCoeffs $n
    $wcrit = Get-ShapiroWilkWcrit05 $n
    if ($null -eq $a -or $st.SS -le 0) {
        return [pscustomobject]@{
            N = $n; W = [double]::NaN; Wcrit05 = $wcrit
            Normal05 = $false; Note = 'n not in 5..12 or zero variance'
        }
    }
    $ord = @($xs | Sort-Object)
    $b = 0.0
    for ($i = 0; $i -lt $a.Count; $i++) {
        $b += $a[$i] * ($ord[$n - 1 - $i] - $ord[$i])
    }
    $w = ($b * $b) / $st.SS
    $ok = $w -ge $wcrit
    [pscustomobject]@{
        N        = $n
        W        = $w
        Wcrit05  = $wcrit
        Normal05 = $ok
        Note     = if ($ok) { 'fail to reject normal (W >= Wcrit 0.05)' } else { 'reject normal (W < Wcrit 0.05)' }
    }
}

function Get-LogGamma([double]$z) {
    $g = 7.0
    $p = @(
        0.99999999999980993, 676.5203681218851, -1259.1392167224028,
        771.32342877765313, -176.61502916214059, 12.507343278686905,
        -0.13857109526572012, 9.9843696540787613e-6, 1.5056327351493116e-7
    )
    if ($z -lt 0.5) {
        return [math]::Log([math]::PI / [math]::Sin([math]::PI * $z)) - (Get-LogGamma (1 - $z))
    }
    $z--
    $x = $p[0]
    for ($i = 1; $i -lt $p.Count; $i++) { $x += $p[$i] / ($z + $i) }
    $t = $z + $g + 0.5
    return 0.5 * [math]::Log(2 * [math]::PI) + ($z + 0.5) * [math]::Log($t) - $t + [math]::Log($x)
}

function Get-RegIncBeta([double]$x, [double]$a, [double]$b) {
    if ($x -le 0) { return 0.0 }
    if ($x -ge 1) { return 1.0 }
    $lbeta = (Get-LogGamma $a) + (Get-LogGamma $b) - (Get-LogGamma ($a + $b))
    $front = [math]::Exp($a * [math]::Log($x) + $b * [math]::Log(1 - $x) - $lbeta) / $a
    if ($x -gt ($a + 1) / ($a + $b + 2)) {
        return 1.0 - (Get-RegIncBeta (1 - $x) $b $a)
    }
    $c = 1.0; $d = 1.0 - ($a + $b) * $x / ($a + 1.0)
    if ([math]::Abs($d) -lt 1e-30) { $d = 1e-30 }
    $d = 1.0 / $d
    $h = $d
    for ($m = 1; $m -le 200; $m++) {
        $m2 = 2 * $m
        $aa = $m * ($b - $m) * $x / (($a + $m2 - 1) * ($a + $m2))
        $d = 1.0 + $aa * $d
        if ([math]::Abs($d) -lt 1e-30) { $d = 1e-30 }
        $c = 1.0 + $aa / $c
        if ([math]::Abs($c) -lt 1e-30) { $c = 1e-30 }
        $d = 1.0 / $d
        $h *= $d * $c
        $aa = -($a + $m) * ($a + $b + $m) * $x / (($a + $m2) * ($a + $m2 + 1))
        $d = 1.0 + $aa * $d
        if ([math]::Abs($d) -lt 1e-30) { $d = 1e-30 }
        $c = 1.0 + $aa / $c
        if ([math]::Abs($c) -lt 1e-30) { $c = 1e-30 }
        $d = 1.0 / $d
        $del = $d * $c
        $h *= $del
        if ([math]::Abs($del - 1.0) -lt 1e-10) { break }
    }
    return $front * $h
}

function Get-StudentTTwoSidedP([double]$tAbs, [double]$df) {
    if ($df -le 0) { return 1.0 }
    $x = $df / ($df + $tAbs * $tAbs)
    $ibet = Get-RegIncBeta $x ($df / 2.0) 0.5
    return [math]::Min(1.0, [math]::Max(0.0, $ibet))
}

# Two-sided P(F > f) for F(d1,d2). F CDF = I_{d2/(d2+d1 f)}(d2/2, d1/2).
function Get-FTwoSidedP([double]$f, [int]$d1, [int]$d2) {
    if ($f -le 0 -or $d1 -lt 1 -or $d2 -lt 1) { return 1.0 }
    if ($f -lt 1.0) {
        $f = 1.0 / $f
        $tmp = $d1; $d1 = $d2; $d2 = $tmp
    }
    $x = $d2 / ($d2 + $d1 * $f)
    $upper = Get-RegIncBeta $x ($d2 / 2.0) ($d1 / 2.0)
    return [math]::Min(1.0, [math]::Max(0.0, 2.0 * $upper))
}

function Get-FTestEqualVar([double[]]$a, [double[]]$b) {
    $sa = Get-UnbiasedStats $a
    $sb = Get-UnbiasedStats $b
    $v1 = $sa.Var; $v2 = $sb.Var
    $d1 = $sa.N - 1; $d2 = $sb.N - 1
    if ($v1 -le 0 -or $v2 -le 0) {
        return [pscustomobject]@{
            F = [double]::NaN; Df1 = $d1; Df2 = $d2; P = 1.0
            Equal05 = $true; Note = 'zero variance'
        }
    }
    $f = $v1 / $v2
    $p = Get-FTwoSidedP $f $d1 $d2
    [pscustomobject]@{
        F       = $f
        Df1     = $d1
        Df2     = $d2
        P       = $p
        Equal05 = ($p -ge 0.05)
        Note    = if ($p -ge 0.05) { 'fail to reject equal var' } else { 'reject equal var - use Welch' }
    }
}

function Get-WelchT([double[]]$a, [double[]]$b) {
    $sa = Get-UnbiasedStats $a
    $sb = Get-UnbiasedStats $b
    $v1 = $sa.Var; $v2 = $sb.Var
    $n1 = $sa.N; $n2 = $sb.N
    $se2 = ($v1 / $n1) + ($v2 / $n2)
    $se = [math]::Sqrt($se2)
    $t = if ($se -gt 0) { ($sb.Mean - $sa.Mean) / $se } else { 0.0 }
    $num = $se2 * $se2
    $den = 0.0
    if ($n1 -gt 1) { $den += [math]::Pow($v1 / $n1, 2) / ($n1 - 1) }
    if ($n2 -gt 1) { $den += [math]::Pow($v2 / $n2, 2) / ($n2 - 1) }
    $df = if ($den -gt 0) { $num / $den } else { [math]::Min($n1, $n2) - 1 }
    $p = Get-StudentTTwoSidedP ([math]::Abs($t)) $df
    [pscustomobject]@{
        Kind  = 'Welch'
        MeanA = $sa.Mean; SdA = $sa.SD; NA = $n1
        MeanB = $sb.Mean; SdB = $sb.SD; NB = $n2
        Delta = $sb.Mean - $sa.Mean
        Pct   = if ($sa.Mean -ne 0) { 100.0 * ($sb.Mean - $sa.Mean) / $sa.Mean } else { [double]::NaN }
        T     = $t
        Df    = $df
        P     = $p
    }
}

function Get-StudentT([double[]]$a, [double[]]$b) {
    $sa = Get-UnbiasedStats $a
    $sb = Get-UnbiasedStats $b
    $n1 = $sa.N; $n2 = $sb.N
    $df = $n1 + $n2 - 2
    $sp2 = if ($df -gt 0) { (($n1 - 1) * $sa.Var + ($n2 - 1) * $sb.Var) / $df } else { 0.0 }
    $se = [math]::Sqrt($sp2 * (1.0 / $n1 + 1.0 / $n2))
    $t = if ($se -gt 0) { ($sb.Mean - $sa.Mean) / $se } else { 0.0 }
    $p = Get-StudentTTwoSidedP ([math]::Abs($t)) $df
    [pscustomobject]@{
        Kind  = 'Student'
        MeanA = $sa.Mean; SdA = $sa.SD; NA = $n1
        MeanB = $sb.Mean; SdB = $sb.SD; NB = $n2
        Delta = $sb.Mean - $sa.Mean
        Pct   = if ($sa.Mean -ne 0) { 100.0 * ($sb.Mean - $sa.Mean) / $sa.Mean } else { [double]::NaN }
        T     = $t
        Df    = $df
        P     = $p
    }
}

function Format-UnbiasedStatsLine([string]$label, $st) {
    '{0}: n={1} mean={2:N3} sd={3:N3} (unbiased n-1) min={4:N3} max={5:N3}' -f `
        $label, $st.N, $st.Mean, $st.SD, $st.Min, $st.Max
}

function Write-MeasureCompare([string]$label, [double[]]$a, [double[]]$b, [string]$nameA, [string]$nameB) {
    $sa = Get-UnbiasedStats $a
    $sb = Get-UnbiasedStats $b
    $na = Get-ShapiroWilk $a
    $nb = Get-ShapiroWilk $b
    $ft = Get-FTestEqualVar $a $b
    $student = Get-StudentT $a $b
    $welch = Get-WelchT $a $b
    $use = if ($ft.Equal05) { $student } else { $welch }
    $sig = if ($use.P -lt 0.05) { 'significant (p<0.05)' } else { 'not significant (p>=0.05)' }
    Write-Host "--- $label ---"
    Write-Host (Format-UnbiasedStatsLine $nameA $sa)
    Write-Host ('  Shapiro-Wilk W={0:N4} Wcrit05={1:N3} -> {2}' -f $na.W, $na.Wcrit05, $na.Note)
    Write-Host (Format-UnbiasedStatsLine $nameB $sb)
    Write-Host ('  Shapiro-Wilk W={0:N4} Wcrit05={1:N3} -> {2}' -f $nb.W, $nb.Wcrit05, $nb.Note)
    Write-Host ('  equal-var F={0:N3} df={1},{2} two-sided p~{3:N4} -> {4}' -f $ft.F, $ft.Df1, $ft.Df2, $ft.P, $ft.Note)
    Write-Host ('  Student t={0:N3} df={1:N1} p~{2:N4}' -f $student.T, $student.Df, $student.P)
    Write-Host ('  Welch   t={0:N3} df~{1:N2} p~{2:N4}' -f $welch.T, $welch.Df, $welch.P)
    Write-Host ('  use {0}; delta({1}-{2})={3:N3} -> {4}' -f $use.Kind, $nameB, $nameA, $use.Delta, $sig)
}

# Back-compat names used by measure-compare-n10.ps1
function Get-Stats([double[]]$xs) { Get-UnbiasedStats $xs }
function Get-Welch([double[]]$a, [double[]]$b) { Get-WelchT $a $b }
