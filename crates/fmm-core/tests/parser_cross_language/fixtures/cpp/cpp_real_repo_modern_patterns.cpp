
#include <memory>
#include <vector>
#include <functional>
#include "event.h"

namespace events {

class EventBus {
public:
    using Handler = std::function<void(const Event&)>;

    void subscribe(Handler handler) {
        handlers_.push_back(std::move(handler));
    }

    void publish(const Event& event) {
        for (auto& handler : handlers_) {
            handler(event);
        }
    }

private:
    std::vector<Handler> handlers_;
};

template <typename T>
class Observable {
public:
    void notify(const T& value) {
        for (auto& obs : observers_) {
            obs(value);
        }
    }

private:
    std::vector<std::function<void(const T&)>> observers_;
};

struct EventData {
    int id;
    std::string payload;
};

} // namespace events
