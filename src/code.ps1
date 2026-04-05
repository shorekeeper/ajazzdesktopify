# --- НАСТРОЙКИ ---

# Путь к папке, где лежат исходные файлы ( "." означает текущая папка)
$SourcePath = ".\" 

# Имя итогового файла
$OutputFileName = "all_code.txt"

# Какие файлы искать (например, "*.cs", "*.py" или "*.*" для всех)
$FileFilter = "*.*" 

# Исключить сам скрипт и итоговый файл, чтобы не было рекурсии
$ScriptFile = $MyInvocation.MyCommand.Source
$OutputFileFullPath = Join-Path (Resolve-Path $SourcePath) $OutputFileName

# --- СКРИПТ ---

# Если старый файл результата существует — удаляем его
if (Test-Path $OutputFileFullPath) {
    Remove-Item $OutputFileFullPath
}

# Получаем список файлов рекурсивно
$Files = Get-ChildItem -Path $SourcePath -Recurse -Include $FileFilter -File | 
    Where-Object { 
        $_.FullName -ne $OutputFileFullPath -and 
        $_.FullName -ne $ScriptFile 
    }

Write-Host "Найдено файлов: $($Files.Count). Начинаю объединение..." -ForegroundColor Cyan

foreach ($File in $Files) {
    try {
        # --- ИЗМЕНЕНИЕ ЗДЕСЬ ---
        # Добавлено -Encoding UTF8 при чтении. 
        # Это заставляет PowerShell правильно интерпретировать кириллицу в современных файлах.
        $Content = Get-Content -Path $File.FullName -Raw -Encoding UTF8 -ErrorAction Stop
        
        # Формируем заголовок и блок
        $Header = "File: $($File.FullName)"
        $BlockStart = "'''" 
        $BlockEnd = "'''"
        $Separator = "`r`n" 

        # Собираем текст для записи
        $FinalText = $Header + $Separator + $BlockStart + $Separator + $Content + $Separator + $BlockEnd + $Separator + $Separator

        # Дописываем в итоговый файл
        Add-Content -Path $OutputFileFullPath -Value $FinalText -Encoding UTF8
    }
    catch {
        Write-Warning "Не удалось прочитать файл (возможно, он занят или это бинарный файл): $($File.FullName)"
    }
}

Write-Host "Готово! Результат сохранен в: $OutputFileFullPath" -ForegroundColor Green