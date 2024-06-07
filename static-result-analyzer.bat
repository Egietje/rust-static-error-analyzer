@echo off

echo Result Analysis Tool by Thomas Kas

set toolchain=nightly-2024-05-18

:: Check whether rustup is installed
rustup --version >nul 2>&1 && ( echo Rustup is installed! ) || ( echo This program requires rustup to be installed! && exit /B )


:: Check whether the correct toolchain is installed
cargo +%toolchain% --version >nul 2>&1 && ( goto installed_toolchain ) || ( goto ask_install_toolchain )


:installed_toolchain
set install_toolchain=n
echo Correct toolchain is installed!
goto after_toolchain


:ask_install_toolchain
set install_toolchain=Y
set /p install_toolchain=Correct toolchain not found, install? (you will be asked if you want to remove it again afterwards) [Y/n] (default - %install_toolchain%): 

if %install_toolchain% == Y ( goto install_toolchain )
if %install_toolchain% == y ( goto install_toolchain )

if %install_toolchain% == N ( goto after_toolchain )
if %install_toolchain% == n ( goto after_toolchain )

goto ask_install_toolchain


:install_toolchain
rustup toolchain install %toolchain%
goto after_toolchain



:after_toolchain
:: Ensure the rustc-dev component is installed
rustup +%toolchain% component add rustc-dev

:: Get the arguments
set input=Cargo.toml
set /p input=Enter relative path to the manifest of the project you want to analyze (default - %input%): 

set output=graph.dot
set /p output=Enter relative path to the output file of the graph (default - %output%): 

set call=


:ask_call
set should_call=n
set /p should_call=Output the call graph instead of propagation chain graph? [Y/n] (default - %should_call%): 

if %should_call% == Y ( goto set_call )
if %should_call% == y ( goto set_call )

if %should_call% == N ( goto after_call )
if %should_call% == n ( goto after_call )


:set_call
set call=--call
goto after_call



:after_call
:: Run the analyzer
echo Building and running analyzer!

cargo +%toolchain% run -- %input% %output% %call%


:: Check whether the toolchain was installed specifically for this, and ask whether to remove it again if it was
if %install_toolchain% == Y ( goto ask_remove_toolchain )
if %install_toolchain% == y ( goto ask_remove_toolchain )

goto end


:ask_remove_toolchain
set remove_toolchain=Y
set /p remove_toolchain=Remove toolchain again? [Y/n] (default - %remove_toolchain%): 

if %remove_toolchain% == Y ( goto remove_toolchain )
if %remove_toolchain% == y ( goto remove_toolchain )

if %remove_toolchain% == N ( goto end )
if %remove_toolchain% == n ( goto end )

goto ask_remove_toolchain


:remove_toolchain
rustup toolchain uninstall %toolchain%



:end
