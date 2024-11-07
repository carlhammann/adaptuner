# Adaptive MIDI tuner

Erstmal nur eine ältere Version mit einer sehr einfachen Logik und einem
rudimentären terminal user interface, die leicht zu verpacken war. 

Kompilieren und laufen lassen mit:
```
nix run .# ./configs/lorem.hjson
```

## TUI

Man sieht ein Quinten-Terzen-Gitter. Zu jedem Zeitpunkt gibt es 
- ein Tonartzentrum, das als "key center" unten im Bild steht,
- 12 mögliche Referenztöne, die etwas heller hervorgehoben sind,
- klingende Töne, die ganz hell hervorgehoben sind, und
- optional ein gerade passendes Akkordmuster ("current fit") mit einem Referenzton ("reference") , das auch unten im Bild steht.

## Tastenbelegungen

- 'q' beendet das Programm
- 'Esc' setzt alles zurück auf die ursprünglichen Werte
- 'Space' verschiebt das Tonartzentrum auf den momentanen Referenzton. Das verschiebt auch die 12 möglichen Referenztöne, sodass sie sich im gleichen Muster wie vorher um das neue Tonartzentrum gruppieren.
- '1' (de)aktiviert 1/4-Komma-mittelttönige Quinten. Default: aus
- '2' (de)aktiviert gleichschwebende Stimmung. Default: aus
- 'p' (de)aktiviert "Pattern Matching" für Akkorde: Wenn deaktiviert, werden nur die hellgrau hervorgehobenen Töne verwendet. Wenn aktiviert, werden die Akkordmuster aus der auf der Kommandozeile gegebenen Konfigurationsdatei verwendet. Default: An
- 't' (de)aktiviert die Anwendung der mit '1' oder '2' aktivierten Stimmung auf harmonische Intervalle. Default: aus
- '-' und '+' zoomen
- Klick auf eine Note fügt diese Note zum Vorrat der 12 möglichen Referenztöne hinzu (und entfernt ihre enharmonische Verwechselung aus dem Vorrat)

