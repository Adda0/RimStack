// What only the application object can change, through Qt's own static
// functions: no Rust reference to the application is held meanwhile.
#pragma once
#include <QtCore/QString>

class QQmlEngine;

// The icon every window carries, given to the desktop with the window.
void setWindowIcon(const QString &resource);

// The "preview" image:// provider; the engine owns it once it is added.
void addPreviewProvider(QQmlEngine &engine);

// New texts take this font family; an empty one gives the system's font.
// Returns the family in use.
QString setFontFamily(const QString &family);
