using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace MyApp.Services
{
    [Serializable]
    public class DataService
    {
        private readonly Dictionary<string, object> _cache;

        public DataService()
        {
            _cache = new Dictionary<string, object>();
        }

        [Required]
        public List<string> Transform(List<string> input)
        {
            return input.Where(s => !string.IsNullOrEmpty(s)).ToList();
        }

        private void Validate(string input)
        {
            if (string.IsNullOrEmpty(input))
                throw new ArgumentException("Input required");
        }
    }

    public interface IRepository<T>
    {
        T FindById(int id);
        IEnumerable<T> FindAll();
        Task SaveAsync(T entity);
    }

    [Obsolete]
    public enum Status
    {
        Active,
        Inactive,
        Pending
    }
}

namespace MyApp.Models
{
    public class ProcessConfig
    {
        public string Name { get; set; }
        public int MaxRetries { get; set; }
        public bool Debug { get; set; }
    }

    internal class InternalHelper
    {
        internal void DoWork() { }
    }
}
