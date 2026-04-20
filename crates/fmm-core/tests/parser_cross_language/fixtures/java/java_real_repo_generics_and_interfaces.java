
import java.util.Optional;
import java.util.function.Predicate;

public interface Validator<T> {
    boolean validate(T item);
    default boolean isValid(T item) {
        return validate(item);
    }
}

public enum Priority {
    LOW, MEDIUM, HIGH, CRITICAL
}

public class StringValidator implements Validator<String> {
    @Override
    public boolean validate(String item) {
        return item != null && !item.isBlank();
    }
}
