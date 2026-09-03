<?php
require __DIR__ . '/Pda.php';
$t = microtime(true);
$n = 200;
for ($i = 0; $i < $n; $i++) { Ed25519::isOnCurve(hash('sha256', "probe$i", true)); }
$ms = (microtime(true) - $t) * 1000 / $n;
printf("isOnCurve: %.2f ms per call\n", $ms);
$t = microtime(true);
$pid = Base58::decode('F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL');
for ($i = 0; $i < 20; $i++) { Pda::findProgramAddress(['site', hash('sha256', "a$i", true)], $pid); }
printf("findProgramAddress: %.1f ms per call\n", (microtime(true) - $t) * 1000 / 20);
