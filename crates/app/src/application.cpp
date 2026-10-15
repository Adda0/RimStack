#include "application.h"

#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>

void setWindowIcon(const QString &resource)
{
    QGuiApplication::setWindowIcon(QIcon(resource));
}

QString setFontFamily(const QString &family)
{
    // Every change of font comes through here, so the first call still
    // finds the font the system gave.
    static const QFont system = QGuiApplication::font();
    QFont font = system;
    if (!family.trimmed().isEmpty())
        font.setFamily(family.trimmed());
    QGuiApplication::setFont(font);
    return font.family();
}
