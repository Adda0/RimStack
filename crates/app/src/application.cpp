#include "application.h"

#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtQuick/QQuickImageProvider>
#include <cstdint>

extern "C" {
    /// Get wide and high of the pack the key names, or false if stale/empty.
    bool rimstack_preview_size(const uint16_t *key_utf16, size_t key_len,
                               uint32_t *wide, uint32_t *high);
    /// Copy the pixel frame into `into`, or false on mismatch/stale.
    bool rimstack_preview_copy(const uint16_t *key_utf16, size_t key_len,
                               uint8_t *into, size_t room);
}
namespace {

/// The rows' small pre-cut copies: looked up where the window asks, cheap enough
/// to be answered on its own thread.
class PreviewProvider : public QQuickImageProvider
{
public:
    PreviewProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}

    QImage requestImage(const QString &key, QSize *size,
                        const QSize &requestedSize) override
    {
        uint32_t wide = 0, high = 0;
        if (size != nullptr) {
            // Decode the key as UTF-16; QString::utf16() includes a null terminator.
            const auto *utf16 = reinterpret_cast<const uint16_t *>(key.utf16());
            if (!rimstack_preview_size(utf16, key.size(), &wide, &high)) {
                return QImage();
            }
            *size = QSize(qint32(wide), qint32(high));
        }
        if (!size->isValid())
            return QImage();
        const auto frame = qint64(size->width()) * size->height() * 4;
        if (frame <= 0)
            return QImage();

        // Allocate an image and copy directly into its pixels.
        QImage picture(size->width(), size->height(),
                       QImage::Format_ARGB32_Premultiplied);
        if (picture.sizeInBytes() != frame) {
            return QImage(); // Unexpected format or size mismatch.
        }
        const auto *utf16 = reinterpret_cast<const uint16_t *>(key.utf16());
        if (!rimstack_preview_copy(utf16, key.size(), picture.bits(),
                                   static_cast<size_t>(frame))) {
            return QImage();
        }
        return picture;
    }
};

} // namespace

void addPreviewProvider(QQmlEngine &engine)
{
    engine.addImageProvider(QStringLiteral("preview"), new PreviewProvider);
}

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
