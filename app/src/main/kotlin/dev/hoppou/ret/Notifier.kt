package dev.hoppou.ret

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.os.Build

/**
 * Manage the notification necessary for the foreground service (mandatory since Android O).
 */
class Notifier(private val context: Service) {

    private var failure = false

    private val notificationManager: NotificationManager
        get() = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    private fun createNotification(failure: Boolean): Notification {
        val builder = createNotificationBuilder()
            .setContentTitle(context.getString(R.string.app_name))
        if (failure) {
            builder.setContentText(context.getString(R.string.relay_disconnected))
                .setSmallIcon(R.drawable.ic_report_problem_24dp)
        } else {
            builder.setContentText(context.getString(R.string.relay_connected))
                .setSmallIcon(R.drawable.ic_usb_24dp)
        }
        return builder.addAction(createStopAction()).build()
    }

    private fun createNotificationBuilder(): Notification.Builder =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(context, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(context)
        }

    fun start() {
        failure = false // reset failure flag
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                context.getString(R.string.app_name),
                NotificationManager.IMPORTANCE_DEFAULT,
            )
            notificationManager.createNotificationChannel(channel)
        }
        context.startForeground(NOTIFICATION_ID, createNotification(false))
    }

    fun stop() {
        @Suppress("DEPRECATION")
        context.stopForeground(true)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            notificationManager.deleteNotificationChannel(CHANNEL_ID)
        }
    }

    fun setFailure(failure: Boolean) {
        if (this.failure != failure) {
            this.failure = failure
            notificationManager.notify(NOTIFICATION_ID, createNotification(failure))
        }
    }

    private fun createStopAction(): Notification.Action {
        val stopIntent = RetService.createStopIntent(context)
        val stopPendingIntent =
            PendingIntent.getService(context, 0, stopIntent, PendingIntent.FLAG_ONE_SHOT)
        // the non-deprecated constructor is not available in API 21
        @Suppress("DEPRECATION")
        val actionBuilder = Notification.Action.Builder(
            R.drawable.ic_close_24dp,
            context.getString(R.string.stop_vpn),
            stopPendingIntent,
        )
        return actionBuilder.build()
    }

    private companion object {
        const val NOTIFICATION_ID = 42
        const val CHANNEL_ID = "ret"
    }
}
